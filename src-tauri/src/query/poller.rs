//! Background polling loop for C-FIND queries and C-MOVE retrieves.
//!
//! Periodically polls Aurabox for pending PACS queries and retrieve
//! requests, executing them via [`crate::query::cfind::execute_cfind`]
//! and [`crate::query::cmove::execute_cmove`] respectively, then posts
//! results (or failures) back to Aurabox.

use crate::aura::aura_api::AuraApi;
use crate::query::cfind::execute_cfind;
use crate::query::cmove::execute_cmove;
use crate::query::models::{PacsQueryRequest, PacsRetrieveRequest};
use crate::store::config::Config;
use crate::{load_config, log_error, log_info};
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::oneshot;

/// Default interval between polls when no queries are active.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

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

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                log_info!("Query poller: shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {
                poll_and_execute(&app, &api).await;
            }
        }
    }
}

/// Single poll cycle: fetch pending queries and retrieves, execute each.
async fn poll_and_execute(app: &AppHandle, api: &AuraApi) {
    let config = load_config(app.clone());

    // Skip polling if no API key is configured
    if config.api_key.is_empty() {
        return;
    }

    // --- C-FIND queries ---
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

    // --- C-MOVE retrieves ---
    let pending_retrieves = match api.fetch_pending_retrieves().await {
        Ok(resp) => resp.retrieves,
        Err(e) => {
            log_error!("Query poller: failed to fetch pending retrieves: {}", e);
            Vec::new()
        }
    };

    if !pending_retrieves.is_empty() {
        log_info!("Query poller: {} pending retrieves", pending_retrieves.len());

        for retrieve in pending_retrieves {
            execute_single_retrieve(api, &config, retrieve).await;
        }
    }
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

    match execute_cfind(calling_ae, &query.service, &query.query_level, &query.filters).await {
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
async fn execute_single_retrieve(
    api: &AuraApi,
    config: &Config,
    retrieve: PacsRetrieveRequest,
) {
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

    match execute_cmove(calling_ae, &pacs_service, &retrieve.study_instance_uid, move_destination).await {
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
