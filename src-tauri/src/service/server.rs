use crate::logger::setup_logger;
use crate::{service};
use service::config::Config;
use service::dicom_server::DICOMServer;
use tokio;


pub async fn start(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    setup_logger();

    // Load configuration
    let mut config = Config::load();

    let host = "127.0.0.1";
    config.set_dicom_config(port, host.to_string());

    // Create DICOM server
    let dicom_server = DICOMServer::new(config);

    // Handle OS signals for graceful shutdown
    tokio::spawn(async move {
        eprintln!("Dicom server message: {:?}", "here");

        // Start the DICOM server
        if let Err(err) = dicom_server.start().await {
            eprintln!("Dicom server error: {:?}", err);
        }
    });

    Ok(())
}
