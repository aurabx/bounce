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
    // JoinHandle of the spawned task that owns the DICOM `TcpListener`.
    // `stop()` awaits this after sending the shutdown signal so the
    // listener is fully dropped (and the port released) before stop
    // returns. Without this, a synchronous shutdown in
    // `RunEvent::ExitRequested` can race the process tear-down and leave
    // the bound port in a state that blocks the next instance from
    // binding (AURA-2291).
    receiver_task: Option<tokio::task::JoinHandle<()>>,
    poller_state: QueryPollerState,
    retry_scheduler_state: RetrySchedulerState,
}

// Initialize the server state in main.rs
pub fn init_server_state() -> ServerState {
    ServerState {
        shutdown_sender: None,
        receiver_task: None,
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

    // Spawn server in a background task. Clone the handle so the outer
    // function can still acquire a fresh `app.state(...)` after the move
    // in order to park the `JoinHandle` below.
    let spawn_app = app.clone();
    let receiver_task = tokio::spawn(async move {
        let app = spawn_app;
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

    // Park the JoinHandle so `stop()` can await the listener actually
    // dropping before it returns.
    {
        let mut state = app_state.lock().await;
        state.receiver_task = Some(receiver_task);
    }

    Ok(())
}

#[allow(unused_assignments)]
pub async fn stop(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app_state = app.state::<Arc<Mutex<ServerState>>>();

    // Take the shutdown senders and the receiver JoinHandle from the
    // state. The handle is awaited below so the `TcpListener` inside the
    // receiver task is fully dropped (and the port released) before this
    // function returns.
    let mut shutdown_sender = None;
    let mut poller_shutdown_sender = None;
    let mut retry_shutdown_sender = None;
    let mut receiver_task = None;
    {
        let mut state = app_state.lock().await;
        shutdown_sender = state.shutdown_sender.take();
        poller_shutdown_sender = state.poller_state.shutdown_sender.take();
        retry_shutdown_sender = state.retry_scheduler_state.shutdown_sender.take();
        receiver_task = state.receiver_task.take();
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

    let sender_existed = shutdown_sender.is_some();
    let mut signal_delivered = false;
    if let Some(sender) = shutdown_sender {
        if sender.send(()).is_ok() {
            signal_delivered = true;
            emit_or_log(&app, "log", "Server stopping...");
        } else {
            emit_or_log(&app, "log", "Server already stopped");
            emit_or_log(&app, "running", false);
            log_info!("Dicom server message: {:?}", "Stopped");
        }
    } else {
        emit_or_log(&app, "log", "No running server to stop");
    }

    // Wait for the spawned receiver task to actually finish so the
    // `TcpListener` is dropped before we return. The accept loop polls
    // for shutdown every second, so a 5 second cap is generous; if it
    // ever blew through we abort the task to force the drop rather than
    // leaving the listener bound (AURA-2291).
    if let Some(handle) = receiver_task {
        if signal_delivered || sender_existed {
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    log_info!("Receiver task exited cleanly");
                }
                Ok(Err(join_err)) => {
                    log_error!("Receiver task join error on stop: {}", join_err);
                }
                Err(_) => {
                    log_error!(
                        "Receiver task did not exit within 5s of shutdown signal; aborting"
                    );
                    abort_handle.abort();
                }
            }
        } else {
            handle.abort();
        }
    }

    Ok(())
}
