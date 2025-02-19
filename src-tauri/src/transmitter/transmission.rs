use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use zip::{
    write::ZipWriter,
    write::SimpleFileOptions,
    CompressionMethod
};
use walkdir::{DirEntry, WalkDir};
use std::io::{Seek, Write};
use std::sync::{Arc};
use reqwest::{Client, Body};
use tokio_util::io::ReaderStream;
use tokio::{fs, fs::File};
use tokio::io::AsyncReadExt;
use tokio::time::{sleep, Instant};
use zip::result::ZipError;
use tokio::sync::{Mutex};
use crate::store::config::Config;
use tokio::sync::oneshot;

struct ScheduledStudy {
    last_received: Instant,
    // We send a signal to the task whenever we want to reset the countdown.
    cancel_tx: oneshot::Sender<()>,
}

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
        let mut map = self.scheduled_studies.lock().await;

        // If we already have a scheduled task for this study, cancel it.
        if let Some(existing) = map.remove(&study_uid) {
            // Tell the old task to cancel. It will exit immediately.

            println!("Found existing study {}", study_uid);
            let _ = existing.cancel_tx.send(());
        }

        // Make a new one-shot channel for the fresh task
        let (tx, rx) = oneshot::channel();
        let scheduled_study = ScheduledStudy {
            last_received: Instant::now(),
            cancel_tx: tx,
        };

        // Store it, so we can cancel later if needed.
        map.insert(study_uid.clone(), scheduled_study);

        println!("Total studies {}", map.len());

        // Clone things needed in the spawned task
        let scheduled_studies = self.scheduled_studies.clone();
        let config = self.config.clone();
        let self_clone = self.clone();

        tokio::spawn(async move {
            tokio::select! {
                // Sleep for 10 seconds...
                _ = sleep(Duration::from_secs(10)) => {
                    println!("Time's up—pushing study {}", study_uid);
                    // Here is where you'd do the actual push, e.g.
                    // if let Err(e) = self_clone.send_study(study_uid.clone(), true).await {
                    //     println!("error running send_study for {}", study_uid);
                    // }

                },
                // OR we get a cancel signal from a newer schedule_study_push call
                _ = rx => {
                    println!("Study {} was reset/canceled before 10s elapsed.", study_uid);
                    return;
                }
            }

            // Whichever branch we took, we're done with this push—remove from the map
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
            println!("Study sent successfully: {:?}", archive_path);
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
                    println!("adding file {path:?} as {name:?} ...");
                    zip.start_file(path_as_string, options)?;
                    let mut f = File::open(path).await?;

                    f.read_to_end(&mut buffer).await?;
                    zip.write_all(&buffer)?;
                    buffer.clear();
                } else if !name.as_os_str().is_empty() {
                    // Only if not root! Avoids path spec / warning
                    // and mapname conversion failed error on unzip
                    println!("adding dir {path_as_string:?} as {name:?} ...");
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