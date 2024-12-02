use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;
use dicom::core::value::{DicomValueType};
use tokio::time::Instant;
use crate::{log_error, log_info, service};
use service::config::Config;
use service::transmission::Transmission;

#[derive(Clone)]
pub struct DICOMServer {
    config: Arc<Config>,
    transmission: Arc<Transmission>,
    study_timers: Arc<Mutex<HashMap<String, Instant>>>,
    study_last_received: Arc<Mutex<HashMap<String, Instant>>>,
}

impl DICOMServer {
    pub fn new(config: Config) -> Self {
        let transmission = Transmission::new(
            config.transmission.api_endpoint.clone(),
            config.transmission.api_key.clone()
        );

        Self {
            config: Arc::new(config),
            transmission: Arc::new(transmission),
            study_timers: Arc::new(Mutex::new(HashMap::new())),
            study_last_received: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Simulate DICOM server start
        log_info!("Starting DICOM server on {}:{}",
            self.config.dicom.host,
            self.config.dicom.port
        );

        // Actual implementation would involve pydicom or similar Rust DICOM library
        Ok(())
    }
    //
    // async fn handle_store(&self, dataset: DataSet) -> Result<(), Box<dyn std::error::Error>> {
    //     let study_id = dataset.get_string("StudyInstanceUID")
    //         .unwrap_or_else(|_| "unknown_study".to_string());
    //     let series_id = dataset.get_string("SeriesInstanceUID")
    //         .unwrap_or_else(|_| "unknown_series".to_string());
    //     let sop_instance = dataset.get_string("SOPInstanceUID")
    //         .unwrap_or_else(|_| "unknown_sop".to_string());
    //
    //     let study_path = Path::new(&self.config.storage.base_dir)
    //         .join(&study_id)
    //         .join(&series_id);
    //
    //     std::fs::create_dir_all(&study_path)?;
    //
    //     let file_path = study_path.join(format!("{}.dcm", sop_instance));
    //
    //     // Save DICOM file
    //     // Actual implementation would use dicom crate's serialization
    //     std::fs::write(&file_path, b"placeholder_dicom_data")?;
    //
    //     // Update last received time for the study
    //     let mut last_received = self.study_last_received.lock().await;
    //     last_received.insert(study_id.clone(), Instant::now());
    //
    //     // Schedule study push
    //     self.schedule_study_push(study_id.clone()).await?;
    //
    //     Ok(())
    // }

    async fn schedule_study_push(&self, study_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let timeout = Duration::from_secs(60);
        let transmission = Arc::clone(&self.transmission);
        let config_clone = Arc::clone(&self.config);
        let study_last_received = Arc::clone(&self.study_last_received);

        tokio::spawn(async move {
            time::sleep(timeout).await;

            let last_received = {
                study_last_received.lock().await
                    .get(&study_id).cloned()
            };

            if let Some(last_time) = last_received {
                let time_since_last = last_time.elapsed();
                log_info!("Last file for study {} was {} seconds ago",
                    study_id, time_since_last.as_secs_f64()
                );
            }

            let study_path = Path::new(&config_clone.storage.base_dir).join(study_id);

            if let Err(e) = transmission.send_archive(
                &study_path,
                config_clone.delete_after_send.unwrap_or(false)
            ).await {
                log_error!("Failed to push study: {}", e);
            }
        });

        Ok(())
    }
}