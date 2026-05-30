use crate::db::database::Database;
use crate::query::poller::{self, QueryPollerState};
use crate::transmitter::retry_scheduler::{self, RetrySchedulerState};
use crate::{load_config, log_error, log_info, receiver};
use local_ip_address::local_ip;
use receiver::dicom_server::DICOMServer;
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio;
use tokio::sync::{oneshot, Mutex};

/// Emit a Tauri event to the frontend, logging any failure instead of
/// panicking. A broken IPC channel must not take down the receiver.
fn emit_or_log<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        log_error!("Failed to emit '{}' event: {}", event, e);
    }
}

// Define a struct to manage server state
pub struct ServerState {
    shutdown_sender: Option<oneshot::Sender<()>>,
    poller_state: QueryPollerState,
    retry_scheduler_state: RetrySchedulerState,
}

// Initialize the server state in main.rs
pub fn init_server_state() -> ServerState {
    ServerState {
        shutdown_sender: None,
        poller_state: poller::init_poller_state(),
        retry_scheduler_state: retry_scheduler::init_retry_scheduler_state(),
    }
}

pub async fn start(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(app.clone());

    // Create DICOM server
    let dicom_server = DICOMServer::new(config.clone(), app.clone());

    // Create a channel for shutdown signal
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Store the shutdown sender in the app state
    let app_state = app.state::<Arc<Mutex<ServerState>>>();
    {
        let mut state = app_state.lock().await;
        state.shutdown_sender = Some(shutdown_tx);

        // Start the query poller alongside the DICOM server
        let (poller_shutdown_tx, poller_shutdown_rx) = oneshot::channel::<()>();
        state.poller_state.shutdown_sender = Some(poller_shutdown_tx);
        poller::start_poller(app.clone(), poller_shutdown_rx);

        // Recover any uploads orphaned by a previous crash/restart (studies
        // left UPLOADING, or stale IN-PROGRESS) by re-queuing them, then start
        // the retry scheduler that drains QUEUED/RETRYING studies.
        let database = app.state::<Database>().inner().clone();
        match database.recover_pending_on_startup().await {
            Ok(count) => {
                if count > 0 {
                    log_info!("Startup recovery: re-queued {} pending studies", count);
                }
            }
            Err(e) => {
                log_error!("Startup recovery failed: {}", e);
            }
        }

        let (retry_shutdown_tx, retry_shutdown_rx) = oneshot::channel::<()>();
        state.retry_scheduler_state.shutdown_sender = Some(retry_shutdown_tx);
        retry_scheduler::start_retry_scheduler(app.clone(), retry_shutdown_rx);
    }

    let local_ip = local_ip().unwrap();
    log_info!("local IP address: {:?}", local_ip);

    // Spawn server in a background task
    tokio::spawn(async move {
        emit_or_log(&app, "log", "Starting server");
        emit_or_log(&app, "running", true);

        let details = json!([{
           "label": "Local endpoint",
           "value": format!("tcp//{}:{}", config.ip_address, config.port)
        },{
           "label": "Network endpoint",
           "value": format!("tcp//{}:{}", local_ip, config.port)
        },{
           "label": "Connected to",
           "value": config.get_api_endpoint()
        },{
           "label": "AE Title",
           "value": config.ae_title
        },{
           "label": "Mode",
           "value": config.mode_from_api_key()
        }]);

        match serde_json::to_string(&details) {
            Ok(details_string) => emit_or_log(&app, "running-details", details_string),
            Err(e) => {
                log_error!("Failed to serialize running details: {}", e);
            }
        }

        // Wrap the server task in a select to handle shutdown
        tokio::select! {
            result = dicom_server.start() => {
                if let Err(err) = result {
                    log_error!("Dicom server error: {:?}", err);
                    emit_or_log(&app, "log", format!("Server error: {}", err));
                    emit_or_log(&app, "error", format!("Server error: {}", err));
                }
            }
            _ = shutdown_rx => {
                log_info!("Shutdown signal received");
                emit_or_log(&app, "log", "Server shutdown requested");
            }
        }

        // Signal that the server has stopped
        emit_or_log(&app, "running", false);
        emit_or_log(&app, "log", "Server stopped");
    });

    Ok(())
}

#[allow(unused_assignments)]
pub async fn stop(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app_state = app.state::<Arc<Mutex<ServerState>>>();

    // Take the shutdown senders from the state
    let mut shutdown_sender = None;
    let mut poller_shutdown_sender = None;
    let mut retry_shutdown_sender = None;
    {
        let mut state = app_state.lock().await;
        shutdown_sender = state.shutdown_sender.take();
        poller_shutdown_sender = state.poller_state.shutdown_sender.take();
        retry_shutdown_sender = state.retry_scheduler_state.shutdown_sender.take();
    }

    // Stop the query poller
    if let Some(sender) = poller_shutdown_sender {
        let _ = sender.send(());
        log_info!("Query poller shutdown signal sent");
    }

    // Stop the retry scheduler
    if let Some(sender) = retry_shutdown_sender {
        let _ = sender.send(());
        log_info!("Retry scheduler shutdown signal sent");
    }

    if let Some(sender) = shutdown_sender {
        // Send the shutdown signal
        if sender.send(()).is_err() {
            emit_or_log(&app, "log", "Server already stopped");
            emit_or_log(&app, "running", false);
            log_info!("Dicom server message: {:?}", "Stopped");
            return Ok(());
        }

        emit_or_log(&app, "log", "Server stopping...");
    } else {
        emit_or_log(&app, "log", "No running server to stop");
    }

    Ok(())
}
