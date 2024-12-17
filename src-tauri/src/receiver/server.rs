use crate::logger::setup_logger;
use crate::{receiver, store};
use receiver::dicom_server::DICOMServer;
use store::config::Config;
use tokio;

pub async fn start(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    setup_logger();

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
