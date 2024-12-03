use crate::{log_info, service};
use tokio;
use service::config::Config;
use service::dicom_server::DICOMServer;
use crate::logger::setup_logger;

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    setup_logger();

    // Load configuration
    let config = Config::load();

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
