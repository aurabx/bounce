//! Background polling loop for C-FIND queries and unified jobs (retrieves + sends).
//!
//! Periodically polls Aurabox for pending PACS queries (legacy endpoint) and
//! pending jobs (unified `/jobs/pending` endpoint covering both inbound
//! C-MOVE retrieves and outbound C-STORE sends), executing each via the
//! appropriate SCU and posting results / progress / failures back to Aurabox.
//!
//! Sends are spawned into bounded tokio tasks (capped at
//! [`MAX_CONCURRENT_SENDS`]) so a single slow remote PACS does not block other
//! work. Retrieves remain sequential — they are inherently serialised by the
//! shared C-STORE SCP receive pipeline anyway.

use crate::aura::aura_api::AuraApi;
use crate::query::cfind::execute_cfind;
use crate::query::cmove::execute_cmove;
use crate::query::models::{Job, PacsQueryRequest, RetrieveJob, SendJob};
use crate::send::worker::execute_send_job;
use crate::store::config::Config;
use crate::{load_config, log_error, log_info};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{oneshot, Semaphore};

/// Default interval between polls when no queries are active.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum number of outbound C-STORE sends that may run concurrently for a
/// single Bounce gateway. Caps association count against any individual
/// remote PACS so an on-prem destination is not overwhelmed.
const MAX_CONCURRENT_SENDS: usize = 2;

/// State for the query poller, holding the shutdown channel sender.
pub struct QueryPollerState {
    pub shutdown_sender: Option<oneshot::Sender<()>>,
}

pub fn init_poller_state() -> QueryPollerState {
    QueryPollerState {
        shutdown_sender: None,
    }
}

/// Start the query poller as a background task.
///
/// The poller will run until the shutdown signal is sent through the
/// [`QueryPollerState`] shutdown channel or the task is dropped.
pub fn start_poller(app: AppHandle, shutdown_rx: oneshot::Receiver<()>) {
    tokio::spawn(async move {
        log_info!("Query poller started");
        run_poll_loop(app, shutdown_rx).await;
        log_info!("Query poller stopped");
    });
}

/// The main poll loop.
async fn run_poll_loop(app: AppHandle, mut shutdown_rx: oneshot::Receiver<()>) {
    let api = AuraApi::new(app.clone());
    let send_semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_SENDS));

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                log_info!("Query poller: shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {
                poll_and_execute(&app, &api, &send_semaphore).await;
            }
        }
    }
}

/// Single poll cycle: fetch pending queries and jobs, execute each.
async fn poll_and_execute(
    app: &AppHandle,
    api: &AuraApi,
    send_semaphore: &Arc<Semaphore>,
) {
    let config = load_config(app.clone());

    // Skip polling if no API key is configured
    if config.api_key.is_empty() {
        return;
    }

    // --- C-FIND queries (legacy endpoint, separate cadence) ---
    let pending_queries = match api.fetch_pending_queries().await {
        Ok(resp) => resp.queries,
        Err(e) => {
            log_error!("Query poller: failed to fetch pending queries: {}", e);
            Vec::new()
        }
    };

    if !pending_queries.is_empty() {
        log_info!("Query poller: {} pending queries", pending_queries.len());
        for query in pending_queries {
            execute_single_query(app, api, &config, query).await;
        }
    }

    // --- Unified jobs endpoint (retrieves + sends) ---
    let pending_jobs = match api.fetch_pending_jobs().await {
        Ok(resp) => resp.jobs,
        Err(e) => {
            log_error!("Query poller: failed to fetch pending jobs: {}", e);
            Vec::new()
        }
    };

    if pending_jobs.is_empty() {
        return;
    }

    let (retrieves, sends) = split_jobs(pending_jobs);

    if !retrieves.is_empty() {
        log_info!("Query poller: {} pending retrieves", retrieves.len());
        for retrieve in retrieves {
            execute_single_retrieve(api, &config, retrieve).await;
        }
    }

    if !sends.is_empty() {
        log_info!(
            "Query poller: {} pending sends (max concurrent {})",
            sends.len(),
            MAX_CONCURRENT_SENDS,
        );
        for send in sends {
            spawn_send(api.clone(), config.ae_title.clone(), send_semaphore.clone(), send);
        }
    }
}

/// Sort the unified jobs payload into per-kind buckets so each can be
/// dispatched with its own concurrency policy.
fn split_jobs(jobs: Vec<Job>) -> (Vec<RetrieveJob>, Vec<SendJob>) {
    let mut retrieves = Vec::new();
    let mut sends = Vec::new();
    for job in jobs {
        match job {
            Job::Retrieve(r) => retrieves.push(r),
            Job::Send(s) => sends.push(s),
        }
    }
    (retrieves, sends)
}

/// Spawn a send into a bounded background task so the poll loop is never
/// blocked by a slow remote PACS.
fn spawn_send(
    api: AuraApi,
    calling_ae: String,
    semaphore: Arc<Semaphore>,
    job: SendJob,
) {
    tokio::spawn(async move {
        // The permit is released when this function returns regardless of the
        // outcome, so a stuck send eventually frees its slot via timeout.
        let _permit = match semaphore.acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                log_error!(
                    "Send {}: semaphore closed, dropping job",
                    job.id,
                );
                return;
            }
        };
        execute_send_job(&api, calling_ae, job).await;
    });
}

/// Execute a single C-FIND query and report results or failure.
async fn execute_single_query(
    _app: &AppHandle,
    api: &AuraApi,
    config: &Config,
    query: PacsQueryRequest,
) {
    let calling_ae = &config.ae_title;

    log_info!(
        "Query poller: executing C-FIND {} against {}@{}:{} (query_level={}) filters={:?}",
        query.id,
        query.service.ae_title,
        query.service.host,
        query.service.port,
        query.query_level,
        query.filters,
    );

    match execute_cfind(
        calling_ae,
        &query.service,
        &query.query_level,
        &query.filters,
    )
    .await
    {
        Ok(results) => {
            let count = results.len();
            log_info!(
                "Query poller: C-FIND {} returned {} results",
                query.id,
                count,
            );

            if let Err(e) = api.post_query_results(&query.id, results).await {
                log_error!(
                    "Query poller: failed to post results for {}: {}",
                    query.id,
                    e,
                );
            }
        }
        Err(error) => {
            log_error!("Query poller: C-FIND {} failed: {}", query.id, error);

            if let Err(e) = api.post_query_failed(&query.id, error).await {
                log_error!(
                    "Query poller: failed to post failure for {}: {}",
                    query.id,
                    e,
                );
            }
        }
    }
}

/// Execute a single C-MOVE retrieve and report completion or failure.
///
/// The C-MOVE tells the PACS to send the study to Bounce's C-STORE SCP.
/// Once the PACS finishes sending, Bounce's DICOM server will have received
/// the files and the normal upload workflow triggers automatically.
async fn execute_single_retrieve(api: &AuraApi, config: &Config, retrieve: RetrieveJob) {
    let calling_ae = &config.ae_title;
    // Use Bounce's own AE title as the move destination so the PACS
    // sends the study to our C-STORE SCP.
    let move_destination = &config.ae_title;
    let pacs_service = retrieve.service.clone().into();

    log_info!(
        "Query poller: executing C-MOVE {} for study {} from {}@{}:{} (destination={})",
        retrieve.id,
        retrieve.study_instance_uid,
        retrieve.service.ae_title,
        retrieve.service.host,
        retrieve.service.port,
        move_destination,
    );

    match execute_cmove(
        calling_ae,
        &pacs_service,
        &retrieve.study_instance_uid,
        move_destination,
    )
    .await
    {
        Ok(()) => {
            log_info!(
                "Query poller: C-MOVE {} completed successfully",
                retrieve.id,
            );

            // Write a retrieve marker file so the upload workflow can
            // include the patient_id in the upload init payload, which
            // allows Aura to auto-match the study to the correct patient.
            if let Some(ref patient_id) = retrieve.patient_id {
                let marker_path = config.resolve_retrieve_marker_path(&retrieve.study_instance_uid);
                log_info!(
                    "Query poller: writing retrieve marker at {:?} for study {} with patient_id {}",
                    marker_path,
                    retrieve.study_instance_uid,
                    patient_id,
                );
                if let Err(e) = std::fs::write(&marker_path, patient_id) {
                    log_error!(
                        "Query poller: failed to write retrieve marker for {}: {}",
                        retrieve.study_instance_uid,
                        e,
                    );
                } else {
                    log_info!(
                        "Query poller: wrote retrieve marker for study {} with patient_id {}",
                        retrieve.study_instance_uid,
                        patient_id,
                    );
                }
            } else {
                log_error!(
                    "Query poller: no patient_id in retrieve {} for study {}, cannot write marker",
                    retrieve.id,
                    retrieve.study_instance_uid,
                );
            }

            if let Err(e) = api.post_retrieve_completed(&retrieve.id).await {
                log_error!(
                    "Query poller: failed to post retrieve completed for {}: {}",
                    retrieve.id,
                    e,
                );
            }
        }
        Err(error) => {
            log_error!("Query poller: C-MOVE {} failed: {}", retrieve.id, error);

            if let Err(e) = api.post_retrieve_failed(&retrieve.id, error).await {
                log_error!(
                    "Query poller: failed to post retrieve failure for {}: {}",
                    retrieve.id,
                    e,
                );
            }
        }
    }
}
