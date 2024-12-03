use crate::{log_info, service};
// use std::env;
use std::process;
// use clap::Parser;
use tokio;
use service::config::Config;
use service::logger::setup_logger;
use service::dicom_server::DICOMServer;

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    setup_logger();

    // Load configuration
    let config = Config::load();

    // Create DICOM server
    let dicom_server = DICOMServer::new(config);

    // Handle OS signals for graceful shutdown
    tokio::spawn(handle_signals());
    tokio::spawn(async move {
        eprintln!("Dicom server message: {:?}", "here");
        // Start the DICOM server
        if let Err(err) = dicom_server.start().await {
            eprintln!("Dicom server error: {:?}", err);
        }
    });

    Ok(())
}


async fn handle_signals() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {
            log_info!("Received SIGINT. Initiating graceful shutdown...");
            process::exit(0);
        }
        _ = sigterm.recv() => {
            log_info!("Received SIGTERM. Initiating graceful shutdown...");
            process::exit(0);
        }
    }
}