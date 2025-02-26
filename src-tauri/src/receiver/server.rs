use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use crate::logger::setup_logger;
use crate::{receiver, AppState};
use receiver::dicom_server::DICOMServer;
use tokio;
use crate::store::config::Config;
use crate::transmitter::manager::TransmissionManager;

pub async fn start(config: Config, app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    // Setup logging
    setup_logger();
    

    // Create DICOM server
    let dicom_server = DICOMServer::new(config, app.clone());

    // let transmission_manager = TransmissionManager::new(config.clone());

    // Create DICOM server
    // let dicom_server = DICOMServer::new(config, app.clone(), Arc::new(transmission_manager));

    // Handle OS signals for graceful shutdown
    // get this out of unsafe
    tokio::spawn(async move {
        eprintln!("Dicom server message: {:?}", "Running");
        app.emit("running", true).unwrap();

        // Start the DICOM server
        if let Err(err) = dicom_server.start().await {
            eprintln!("Dicom server error: {:?}", err);
            app.emit("running", false).unwrap();
        }
    });


    Ok(())
}

pub(crate) async fn stop(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    // let state = app.state::<ServerState>();
    // let sender_state = Arc::new(state);

    app.emit("running", false).unwrap();

    eprintln!("Dicom server message: {:?}", "Stopped");

    //sender_state.stop_sender.send("stopped").expect("TODO: panic message");

    // if let Some(stop_sender) = &state {
    //     // Send stop signal
    //     stop_sender.send("0").map_err(|_| "Failed to send stop signal")?
    // }

    Ok(())
}

