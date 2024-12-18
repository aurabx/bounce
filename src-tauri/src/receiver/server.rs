use tauri::{AppHandle, Emitter, Manager};
use crate::logger::setup_logger;
use crate::{receiver, store};
use receiver::dicom_server::DICOMServer;
use store::config::Config;
use tokio;
use std::sync::{Arc, Mutex};
use tokio::{sync::oneshot, task::JoinHandle};
use tokio::sync::oneshot::Sender;



struct ServerState {
    stop_sender: Sender<&'static str>,
}

pub async fn start(config: Config, app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    // Setup logging
    setup_logger();

    // Create a channel for stopping the server
    let (stop_sender, stop_receiver) = oneshot::channel();

    // Create DICOM server
    let dicom_server = DICOMServer::new(config);

    // Create a shared state to keep track of the server task
    let server_task = Arc::new(Mutex::new(None));
    let server_task_clone = Arc::clone(&server_task);

    let app_clone = app.clone();

    // Spawn the server task
    let task_handle: JoinHandle<()> = tokio::spawn(async move {
        eprintln!("Dicom server message: {:?}", "Running");
        app.clone().emit("running", true).unwrap();

        // Create a tokio select to wait for either server completion or stop signal
        tokio::select! {
            result = dicom_server.start() => {
                if let Err(err) = result {
                    eprintln!("Dicom server error: {:?}", err);
                    app.clone().emit("running", false).unwrap();
                }
            }
            _ = stop_receiver => {
                // Graceful shutdown logic
                eprintln!("Stopping DICOM server");
                // dicom_server.stop().await; // Assuming you have a stop method
                app.clone().emit("running", false).unwrap();
            }
        }
    });

    // Store the task handle
    *server_task_clone.lock().unwrap() = Some(task_handle);

    // Store stop sender in app state if you want to keep track of it
    app_clone.manage(ServerState {
        stop_sender
    });

    Ok(())

    // // Handle OS signals for graceful shutdown
    // tokio::spawn(async move {
    //     eprintln!("Dicom server message: {:?}", "Running");
    //     app.emit("running", true).unwrap();
    //
    //     // Start the DICOM server
    //     if let Err(err) = dicom_server.start().await {
    //         eprintln!("Dicom server error: {:?}", err);
    //         app.emit("running", false).unwrap();
    //     }
    // });
    //
    // Ok(())
}

pub(crate) async fn stop(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    let state = app.state::<ServerState>();
    let sender_state = Arc::new(state);


    //sender_state.stop_sender.send("stopped").expect("TODO: panic message");

    // if let Some(stop_sender) = &state {
    //     // Send stop signal
    //     stop_sender.send("0").map_err(|_| "Failed to send stop signal")?
    // }

    Ok(())
}