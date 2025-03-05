use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use crate::{log_error, log_info, receiver};
use receiver::dicom_server::DICOMServer;
use tokio;
use tokio::sync::{oneshot, Mutex};
use crate::store::config::Config;

// Define a struct to manage server state
pub struct ServerState {
    shutdown_sender: Option<oneshot::Sender<()>>,
}

// Initialize the server state in main.rs
pub fn init_server_state() -> ServerState {
    ServerState {
        shutdown_sender: None,
    }
}

pub async fn start(config: Config, app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    // Create DICOM server
    let dicom_server = DICOMServer::new(config, app.clone());

    // Create a channel for shutdown signal
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Store the shutdown sender in the app state
    let app_state = app.state::<Arc<Mutex<ServerState>>>();
    {
        let mut state = app_state.lock().await;
        state.shutdown_sender = Some(shutdown_tx);
    }

    // Spawn server in a background task
    tokio::spawn(async move {
        app.emit("log", "Starting server").unwrap();
        app.emit("running", true).unwrap();

        // Wrap the server task in a select to handle shutdown
        tokio::select! {
            result = dicom_server.start() => {
                if let Err(err) = result {
                    log_error!("Dicom server error: {:?}", err);
                    app.emit("log", format!("Server error: {}", err)).unwrap();
                }
            }
            _ = shutdown_rx => {
                log_info!("Shutdown signal received");
                app.emit("log", "Server shutdown requested").unwrap();
            }
        }

        // Signal that the server has stopped
        app.emit("running", false).unwrap();
        app.emit("log", "Server stopped").unwrap();
    });

    Ok(())
}


#[allow(unused_assignments)]
pub async fn stop(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app_state = app.state::<Arc<Mutex<ServerState>>>();

    // Take the shutdown sender from the state

    let mut shutdown_sender = None;
    {
        let mut state = app_state.lock().await;
        shutdown_sender = state.shutdown_sender.take();
    }

    if let Some(sender) = shutdown_sender {
        // Send the shutdown signal
        if let Err(_) = sender.send(()) {
            app.emit("log", "Server already stopped").unwrap();
            app.emit("running", false).unwrap();
            log_info!("Dicom server message: {:?}", "Stopped");
            return Ok(());
        }

        app.emit("log", "Server stopping...").unwrap();
    } else {
        app.emit("log", "No running server to stop").unwrap();
    }

    Ok(())
}

