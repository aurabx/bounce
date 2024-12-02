use crate::{log_info, service};
// use std::env;
use std::process;
// use clap::Parser;
use tokio;
use service::config::Config;
use service::logger::setup_logger;
use service::dicom_server::DICOMServer;


#[tokio::main]
pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    service::logger::setup_logger();


    // Override environment variables
    // env::set_var("DICOM_PORT", config.port.to_string());
    // env::set_var("API_ENDPOINT", config.destination);
    // env::set_var("API_KEY", config.api_key);
    // env::set_var("STORAGE_DIR", config.storage);

    // Load configuration
    let config = Config::load();

    // Create DICOM server
    let dicom_server = DICOMServer::new(config);

    // Handle OS signals for graceful shutdown
    tokio::spawn(handle_signals());

    // Start the DICOM server
    dicom_server.start().await?;

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