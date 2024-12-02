use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Read};
use std::time::Duration;

use anyhow::{Result, Context};
use reqwest::Client;
use zip::write::SimpleFileOptions;
use sha2::{Sha256, Digest};
use std::io::Write;

pub struct Transmission {
    client: Client,
    api_endpoint: String,
    api_key: String,
}



impl Transmission {
    pub fn new(api_endpoint: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_endpoint,
            api_key,
        }
    }

    pub async fn send_archive(
        &self,
        study_path: &Path,
        delete_after_send: bool
    ) -> Result<()> {
        let archive_path = self.compress_study(study_path)?;

        // Compute checksum
        let checksum = self.compute_checksum(&archive_path)?;

        // Send archive
        let file = File::open(&archive_path)?;
        let response = self.client.post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/octet-stream")
            .header("X-Checksum", checksum)
            .timeout(Duration::from_secs(30))
            .body(file)
            .send()
            .await?;

        if response.status().is_success() {
            if delete_after_send {
                self.delete_local_study_files(study_path)?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to send archive. Status: {}", response.status()))
        }
    }

    fn compress_study(&self, study_path: &Path) -> Result<PathBuf> {
        let archive_path = study_path.with_extension("zip");
        let file = std::fs::File::create(&archive_path)?;
        let mut zip = zip::ZipWriter::new(file);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            // files over u32::MAX require this flag set.
            .large_file(true)
            .unix_permissions(0o755);

        zip.start_file_from_path(archive_path.as_path(), Default::default())?;

        zip.finish()?;

        Ok(archive_path)
    }

    fn delete_local_study_files(&self, study_path: &Path) -> Result<()> {
        fs::remove_dir_all(study_path)
            .context("Failed to delete local study files")
    }

    fn compute_checksum(&self, file_path: &Path) -> Result<String> {
        let mut file = File::open(file_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 4096];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }
}