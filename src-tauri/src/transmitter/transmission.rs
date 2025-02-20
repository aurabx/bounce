use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::{
    write::ZipWriter,
    write::SimpleFileOptions,
    CompressionMethod
};
use walkdir::{DirEntry, WalkDir};
use std::io::{Seek, Write};
use reqwest::{Client, Body};
use tokio_util::io::ReaderStream;
use tokio::{fs, fs::File};
use tokio::io::AsyncReadExt;

use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, oneshot};
use tokio::time::{sleep, Duration, Instant};

use zip::result::ZipError;
use crate::log_info;
use crate::store::config::Config;

#[derive(Debug)]
struct ScheduledStudy {
    last_received: Instant,
    // We send a signal to the task whenever we want to reset the countdown.
    cancel_tx: oneshot::Sender<()>,
}

#[derive(Clone, Debug)]
pub struct Transmission {
    client: Client,
    config: Arc<Config>,
    scheduled_studies: Arc<Mutex<HashMap<String, ScheduledStudy>>>,
}

impl Transmission {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config: Arc::new(config),
            scheduled_studies: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Schedule pushing the study in 10 seconds—debouncing repeated calls.
    /// If another call comes in for the same study before the 10s ends,
    /// we cancel and restart the countdown.
    pub async fn schedule_study_push(&self, study_uid: String) -> anyhow::Result<()> {
        // Acquire the map lock
        let mut map = self.scheduled_studies.lock().await;

        // If there’s already a scheduled task for this study, cancel it.
        // This ensures the old countdown is invalidated.
        if let Some(existing) = map.remove(&study_uid) {
            let _ = existing.cancel_tx.send(());
        }

        // Create a fresh oneshot channel so we can signal cancellation
        let (tx, rx) = oneshot::channel();
        let scheduled_study = ScheduledStudy {
            last_received: Instant::now(),
            cancel_tx: tx,
        };

        // Store the new ScheduledStudy for this UID
        map.insert(study_uid.clone(), scheduled_study);

        // Clone what we need for the spawned task
        let scheduled_studies = Arc::clone(&self.scheduled_studies);

        // Spawn the debounce countdown in a background task
        tokio::spawn(async move {
            tokio::select! {
                // Wait for 10 seconds
                _ = sleep(Duration::from_secs(5)) => {
                    // 10 seconds have passed with no new call for this UID
                    log_info!("Time's up—pushing study {}", study_uid);
                    // do the actual push logic here, e.g. `self_clone.send_study(...).await`
                    let study_uid = study_uid.clone();

                    tokio::spawn(async move {
                        sleep(Duration::from_secs(30)).await;
                        log_info!("pretended this might take 60 secs to complete {}", study_uid);
                    });
                },
                // OR we get a cancellation signal because schedule_study_push was called again
                _ = rx => {
                    log_info!("Study {} was reset/canceled before 10s elapsed.", study_uid);
                    return;
                }
            }


            // Either we pushed or canceled, so remove this entry from the map
            let mut map = scheduled_studies.lock().await;
            map.remove(&study_uid);
        });

        Ok(())
    }

    pub async fn send_study(
        &self,
        study_uid: String,
        delete_after_send: bool
    ) -> Result<()> {

        // Actually push the study (you’ll have to adapt to your code)
        let out_dir = PathBuf::from("./tmp");
        let mut file_path = out_dir.clone();
        file_path.push(study_uid.trim_end_matches('\0').to_string());

        let study_path = file_path.as_path();

        println!("Preparing to send study: {:?}", study_path);

        let archive_path = self.compress_study(study_path).await?;

        // Send archive
        let file = File::open(&archive_path).await?;
        // let file_size = file.metadata()?.len();
        let stream = ReaderStream::new(file);
        let body_stream = Body::wrap_stream(stream);

        let response = self.client.post(&self.config.api_endpoint)
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .header("Content-Type", "application/octet-stream")
            .timeout(Duration::from_secs(30))
            .body(body_stream)
            .send()
            .await?;

        if response.status().is_success() {
            log_info!("Study sent successfully: {:?}", archive_path);
            if delete_after_send {
                self.delete_local_study_files(study_path).await?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to send archive. Status: {}", response.status()))
        }
    }


    pub async fn zip_folder<T>(
        &self,
        it: &mut dyn Iterator<Item = DirEntry>,
            prefix: &Path,
            writer: T
        ) -> anyhow::Result<()>
        where
            T: Write + Seek,
        {
            let mut zip = ZipWriter::new(writer);
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o755);

            let prefix = Path::new(prefix);
            let mut buffer = Vec::new();
            for entry in it {
                let path = entry.path();
                let name = path.strip_prefix(prefix).unwrap();
                let path_as_string = name
                    .to_str()
                    .map(str::to_owned)
                    .with_context(|| format!("{name:?} Is a Non UTF-8 Path"))?;

                // Write file or directory explicitly
                // Some unzip tools unzip files with directory paths correctly, some do not!
                if path.is_file() {
                    log_info!("adding file {path:?} as {name:?} ...");
                    zip.start_file(path_as_string, options)?;
                    let mut f = File::open(path).await?;

                    f.read_to_end(&mut buffer).await?;
                    zip.write_all(&buffer)?;
                    buffer.clear();
                } else if !name.as_os_str().is_empty() {
                    // Only if not root! Avoids path spec / warning
                    // and mapname conversion failed error on unzip
                    log_info!("adding dir {path_as_string:?} as {name:?} ...");
                    zip.add_directory(path_as_string, options)?;
                }
            }
            zip.finish()?;
            Ok(())
        }


    async fn compress_study(&self, study_path: &Path) -> Result<PathBuf> {
        let archive_path = study_path.with_extension("zip");


        if !Path::new(study_path).is_dir() {
            return Err(ZipError::FileNotFound.into());
        }

        let path = Path::new(&archive_path);
        let file = std::fs::File::create(path)?;

        let walkdir = WalkDir::new(study_path);
        let it = walkdir.into_iter();

        self.zip_folder(&mut it.filter_map(|e| e.ok()), study_path, file).await?;

        Ok(archive_path)

    }

    async fn delete_local_study_files(&self, study_path: &Path) -> Result<()> {
        fs::remove_dir_all(study_path)
            .await
            .context("Failed to delete local study files")?;

        Ok(())
    }




}