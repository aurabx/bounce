//! Background polling loop for C-FIND queries.
//!
//! Periodically polls Aurabox for pending PACS queries, executes them via
//! [`crate::query::cfind::execute_cfind`], and posts results (or failures)
//! back to Aurabox.

use crate::aura::aura_api::AuraApi;
use crate::query::cfind::execute_cfind;
use crate::query::models::PacsQueryRequest;
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

/// Single poll cycle: fetch pending queries and execute each one.
async fn poll_and_execute(app: &AppHandle, api: &AuraApi) {
    let config = load_config(app.clone());

    // Skip polling if no API key is configured
    if config.api_key.is_empty() {
        return;
    }

    let pending = match api.fetch_pending_queries().await {
        Ok(resp) => resp.queries,
        Err(e) => {
            // Log at debug level to avoid spamming when the endpoint doesn't exist yet
            log_error!("Query poller: failed to fetch pending queries: {}", e);
            return;
        }
    };

    if pending.is_empty() {
        return;
    }

    log_info!("Query poller: {} pending queries", pending.len());

    for query in pending {
        execute_single_query(app, api, &config, query).await;
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

    match execute_cfind(calling_ae, &query.service, &query.filters).await {
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
