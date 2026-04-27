//! Per-job worker for outbound C-STORE sends.
//!
//! Orchestrates a single [`SendJob`] from start to finish:
//!   1. fetch the study bytes from Uhura via WADO-RS
//!   2. filter by series UIDs if the job specifies a subset
//!   3. open a DIMSE association and C-STORE each instance to the destination
//!   4. report progress and terminal state back to Aura
//!
//! Errors are bubbled to Aura via `post_send_failed`; partial successes are
//! still reported as a failure (with a summary error) since v1 treats a send
//! as atomic from the user's perspective.

use crate::aura::aura_api::AuraApi;
use crate::query::cstore::execute_cstore;
use crate::query::models::SendJob;
use crate::send::uhura_client::UhuraClient;
use crate::{log_error, log_info};
use dicom::dictionary_std::tags;
use dicom::object::{FileDicomObject, InMemDicomObject};

/// Execute one outbound send end-to-end, reporting status back to Aura.
///
/// Returns `Ok(())` regardless of the underlying outcome — terminal state is
/// posted to Aura, and the caller (the poller) does not need to track
/// individual job results.
pub async fn execute_send_job(api: &AuraApi, calling_ae: String, job: SendJob) {
    let send_id = job.id.clone();

    log_info!(
        "C-STORE send {}: starting (study={}, destination={}@{}:{}{})",
        send_id,
        job.study_instance_uid,
        job.destination.ae_title,
        job.destination.host,
        job.destination.port,
        match &job.series_uids {
            Some(uids) => format!(", {} series", uids.len()),
            None => " (all series)".to_string(),
        },
    );

    // Phase 1: fetch from Uhura.
    let uhura = UhuraClient::new();
    let mut instances = match uhura
        .fetch_study(&job.source.wado_base_url, &job.source.study_path, &job.source.jwt)
        .await
    {
        Ok(insts) => insts,
        Err(e) => {
            let msg = format!("WADO-RS fetch failed: {}", e);
            log_error!("C-STORE send {}: {}", send_id, msg);
            if let Err(post_err) = api.post_send_failed(&send_id, msg).await {
                log_error!(
                    "C-STORE send {}: failed to post failure to Aura: {}",
                    send_id,
                    post_err,
                );
            }
            return;
        }
    };

    // Phase 2: optional series filter.
    if let Some(series_uids) = &job.series_uids {
        let before = instances.len();
        instances = filter_to_series(instances, series_uids);
        log_info!(
            "C-STORE send {}: filtered {} -> {} instances by series subset",
            send_id,
            before,
            instances.len(),
        );
    }

    let total = instances.len() as u32;

    if instances.is_empty() {
        let msg = "no instances matched the requested series subset".to_string();
        log_error!("C-STORE send {}: {}", send_id, msg);
        if let Err(post_err) = api.post_send_failed(&send_id, msg).await {
            log_error!(
                "C-STORE send {}: failed to post failure to Aura: {}",
                send_id,
                post_err,
            );
        }
        return;
    }

    // Report the now-known total before the long-running C-STORE batch starts
    // so the UI can switch from a spinner to "0 of N" immediately.
    if let Err(e) = api.post_send_progress(&send_id, 0, Some(total)).await {
        log_error!(
            "C-STORE send {}: failed to post initial progress: {}",
            send_id,
            e,
        );
    }

    // Phase 3: C-STORE batch.
    let pacs_service = job.destination.clone().into();
    let report_result = execute_cstore(&calling_ae, &pacs_service, &instances).await;

    match report_result {
        Ok(report) => {
            let succeeded = report.successes.len() as u32;
            let _ = api
                .post_send_progress(&send_id, succeeded, Some(total))
                .await;

            if report.failures.is_empty() {
                log_info!(
                    "C-STORE send {}: completed; {}/{} instances stored",
                    send_id,
                    succeeded,
                    total,
                );
                if let Err(e) = api.post_send_completed(&send_id).await {
                    log_error!(
                        "C-STORE send {}: failed to post completion: {}",
                        send_id,
                        e,
                    );
                }
            } else {
                let summary = summarise_failures(&report);
                log_error!(
                    "C-STORE send {}: {} of {} instances failed; reporting send as failed",
                    send_id,
                    report.failures.len(),
                    total,
                );
                if let Err(e) = api.post_send_failed(&send_id, summary).await {
                    log_error!(
                        "C-STORE send {}: failed to post failure: {}",
                        send_id,
                        e,
                    );
                }
            }
        }
        Err(e) => {
            log_error!("C-STORE send {}: association-level failure: {}", send_id, e);
            if let Err(post_err) = api.post_send_failed(&send_id, e).await {
                log_error!(
                    "C-STORE send {}: failed to post failure to Aura: {}",
                    send_id,
                    post_err,
                );
            }
        }
    }
}

fn filter_to_series(
    instances: Vec<FileDicomObject<InMemDicomObject>>,
    series_uids: &[String],
) -> Vec<FileDicomObject<InMemDicomObject>> {
    instances
        .into_iter()
        .filter(|obj| match obj.element(tags::SERIES_INSTANCE_UID) {
            Ok(elem) => match elem.to_str() {
                Ok(s) => series_uids.iter().any(|u| u == s.trim_end_matches('\0')),
                Err(_) => false,
            },
            Err(_) => false,
        })
        .collect()
}

fn summarise_failures(report: &crate::query::cstore::SendReport) -> String {
    let total = report.total();
    let failed = report.failures.len();

    // Surface the first failure's message — usually the most diagnostic.
    let first = report
        .failures
        .first()
        .map(|f| {
            let comment = f
                .error_comment
                .clone()
                .unwrap_or_else(|| "no error comment".to_string());
            format!(
                "first failure: status 0x{:04X} on {} ({})",
                f.status, f.sop_instance_uid, comment,
            )
        })
        .unwrap_or_default();

    format!(
        "C-STORE batch failed: {} of {} instances rejected. {}",
        failed, total, first,
    )
}
