use crate::aura::aura_api::AuraApi;
use crate::log_info;
use crate::store::config::Config;
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::{multipart, Client};
use serde_json::Value;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, sync::Arc};
use tokio::io::AsyncReadExt;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{sleep, Duration};
use tokio::{fs, fs::File};
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};
use zip::result::ZipError;
use zip::{write::SimpleFileOptions, write::ZipWriter, CompressionMethod};
// use tokio_util::io::ReaderStream;

#[derive(Debug)]
pub struct ScheduledStudy {
    // last_received: Instant,
    // We send a signal to the task whenever we want to reset the countdown.
    cancel_tx: oneshot::Sender<()>,
}

#[derive(Debug, Clone)]
pub struct Transmission {
    client: Client,
    scheduled_studies: Arc<Mutex<HashMap<String, ScheduledStudy>>>,
    aura_api: Arc<AuraApi>,
    config: Config,
}

impl Transmission {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            scheduled_studies: Arc::new(Mutex::new(HashMap::new())),
            aura_api: Arc::new(AuraApi::new(config.clone())),
            config,
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
            // last_received: Instant::now(),
            cancel_tx: tx,
        };

        // Store the new ScheduledStudy for this UID
        map.insert(study_uid.clone(), scheduled_study);

        // Clone what we need for the spawned task
        let scheduled_studies = Arc::clone(&self.scheduled_studies);
        let study_uid_clone = study_uid.clone();
        let self_clone = self.clone();

        // Spawn the debounce countdown in a background task
        tauri::async_runtime::spawn(async move {
            tokio::select! {
                // Wait for 10 seconds
                _ = sleep(Duration::from_secs(10)) => {
                    // 5 seconds have passed with no new call for this UID
                    log_info!("Time's up -> pushing study {}", study_uid_clone);

                    // do the actual push logic here, e.g. `self_clone.send_study(...).await`
                    let study_uid = study_uid.clone();

                    // tauri::async_runtime::spawn(async move {
                    //     sleep(Duration::from_secs(30)).await;
                    //     log_info!("moved run {}", study_uid_clone);
                    // });

                    let _ = self_clone.send_study(study_uid, false).await;
                    // sleep(Duration::from_secs(30)).await;
                    // log_info!("pretended this might take 30 secs to complete {}", study_uid_clone);
                },
                // OR we get a cancellation signal because schedule_study_push was called again
                _ = rx => {
                    log_info!("Study {} was reset/canceled before 10s elapsed.", study_uid_clone);
                    return;
                }
            }

            // Either we pushed or canceled, so remove this entry from the map
            let mut map = scheduled_studies.lock().await;
            map.remove(&study_uid);
        });

        Ok(())
    }

    pub async fn delete_study(&self, study_uid: String) -> Result<()> {
        self.delete_local_study_files(study_uid.clone()).await?;
        self.delete_local_compressed_study(study_uid.clone())
            .await?;

        log_info!("Deleted local study files and archive for {}", study_uid);

        Ok(())
    }

    pub async fn send_study(&self, study_uid: String, delete_after_send: bool) -> Result<()> {
        let upload_id = Uuid::new_v4();

        log_info!(
            "Starting send_study {} for upload: {}",
            &study_uid,
            &upload_id
        );

        let study_path = self.resolve_study_path(&study_uid);

        log_info!("Preparing to send study: {:?}", &study_path);

        let archive_path = self.compress_study(study_path.clone()).await?;
        log_info!("Preparing to send study zip: {:?}", archive_path);


        // === Create an Assembly on Transloadit, get the TUS URL back
        let assembly = self.create_transloadit_assembly(&upload_id).await?;
        log_info!("Got TUS URL: {}", assembly.get("tus_url").unwrap());

        self.aura_api
            .upload_start(
                study_uid.clone(),
                assembly.get("signature").unwrap().to_string(),
                upload_id.to_string(),
            )
            .await
            .expect("Error sending upload start api message");
        log_info!("Sent upload start to aura");
        
        // === Upload via TUS
        self.upload_via_tus(&assembly, &archive_path).await?;
        log_info!(
            "Study sent successfully via TUS to {}",
            assembly.get("tus_url").unwrap()
        );

        self.aura_api
            .upload_save(
                upload_id.clone().to_string(),
                assembly.get("assembly_id").unwrap().to_string(),
                "update"
            )
            .await
            .expect("Error sending upload update api message");

        log_info!("Sent upload update to aura");


        self.aura_api
            .upload_save(
                upload_id.clone().to_string(),
                assembly.get("assembly_id").unwrap().to_string(),
                "complete"
            )
            .await
            .expect("Error sending complete update api message");

        log_info!("Sent upload complete to aura");

        // Optionally, delete local study if requested
        if delete_after_send {
            self.delete_study(study_uid).await?;
        }

        Ok(())
    }

    fn resolve_study_path(&self, study_uid: &String) -> PathBuf {
        // Actually push the study (you’ll have to adapt to your code)
        let mut file_path = PathBuf::from(&self.config.base_dir);
        file_path.push(study_uid.trim_end_matches('\0').to_string());

        file_path
    }

    pub async fn zip_folder<T, I>(&self, it: I, prefix: &Path, writer: T) -> anyhow::Result<()>
    where
        T: Write + Seek,
        I: Iterator<Item = DirEntry> + Send,
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

    async fn compress_study(&self, study_path: PathBuf) -> Result<PathBuf> {
        let study_path = study_path.as_path();
        let archive_path = study_path.with_extension("zip");

        if !Path::new(study_path).is_dir() {
            return Err(ZipError::FileNotFound.into());
        }

        let path = Path::new(&archive_path);
        let file = std::fs::File::create(path)?;

        let walkdir = WalkDir::new(study_path);
        let it = walkdir.into_iter().filter_map(|e| e.ok());

        // Pass the iterator by value instead of a mutable reference
        self.zip_folder(it, study_path, file).await?;

        log_info!("zip path {archive_path:?}");

        Ok(archive_path)
    }

    async fn delete_local_study_files(&self, study_uid: String) -> Result<()> {
        let study_path = self.resolve_study_path(&study_uid);

        fs::remove_dir_all(study_path.as_path())
            .await
            .context("Failed to delete local study files")?;

        Ok(())
    }

    async fn delete_local_compressed_study(&self, study_uid: String) -> Result<()> {
        let archive_path = self.resolve_study_path(&study_uid).with_extension("zip");

        fs::remove_file(archive_path.as_path())
            .await
            .context("Failed to delete local study files")?;

        Ok(())
    }

    /// Create a Transloadit Assembly and return its TUS upload URL.
    async fn create_transloadit_assembly(&self, upload_id: &Uuid) -> Result<Value> {
        let signature_result = self.aura_api.generate_signature().await?;
        let signature = signature_result
            .get("signature")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        log_info!("signature {:#?}", &signature);
        log_info!(
            "params {:#?}",
            signature_result
                .get("params")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        );

        let form = multipart::Form::new()
            .text(
                "params",
                signature_result
                    .get("params")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
            .text("signature", signature.clone())
            .text("mode", "supplier")
            .text("upload_id", upload_id.to_string())
            .text("num_expected_upload_files", "1");

        // The Transloadit docs say you can send:
        // POST to https://api2.transloadit.com/assemblies
        // with a JSON body that has "params" as a JSON-encoded string, or
        // that you can pass it as top-level JSON. This snippet uses
        // top-level "params" JSON for convenience:
        let resp = self
            .client
            .post("https://api2.transloadit.com/assemblies")
            // .header("Content-Type", "multipart/form-data")
            // .json(&assembly_params)
            .multipart(form)
            .send()
            .await
            .context("Failed POST to Transloadit /assemblies")?;

        if !resp.status().is_success() {
            let status = resp.status();

            log_info!("Could not create Transloadit assembly {}", resp.status());

            let resp_json: Value = resp.json().await?;
            log_info!("{:#?}", resp_json);

            return Err(anyhow!(
                "Could not create Transloadit assembly: status={:?}",
                status
            ));
        }

        // The Transloadit response includes "tus_url" - parse it out:
        let mut resp_json: Value = resp
            .json()
            .await
            .context("Failed to parse create-assembly JSON")?;

        log_info!("{:#?}", resp_json);

        if let Some(tus_url) = resp_json.get("tus_url") {
            // `tus_url` exists, do something with it
            println!("tus_url exists: {:?}", tus_url);
        } else {
            // `tus_url` does not exist
            log_info!("Missing tus_url in Transloadit assembly response");

            return Err(anyhow!("Missing tus_url in Transloadit assembly response"));
        }

        if let Some(obj) = resp_json.as_object_mut() {
            obj.insert(
                "signature".to_string(),
                Value::String(signature.to_string()),
            );
        } else {
            eprintln!("resp_json is not an object and cannot have key-value pairs added.");
        }

        println!("tus response exists: {:?}", resp_json.to_string());

        Ok(resp_json)
    }

    /// A simple TUS upload example.
    /// In a real TUS workflow, you might handle chunking, resume, or partial patches,
    /// but here we just do a single-chunk approach or a small streaming approach.
    async fn upload_via_tus(&self, assembly: &Value, file_path: &Path) -> Result<()> {
        let file_size = fs::metadata(file_path)
            .await
            .context("Could not get metadata for file")?
            .len();

        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.dcm.zip");

        let tus_url = assembly.get("tus_url").unwrap().as_str().unwrap();
        let assembly_url = assembly.get("assembly_url").unwrap().as_str().unwrap();

        let metadata_fields = vec![
            ("name", file_name),
            ("type", "application/zip"),
            ("assembly_url", assembly_url),
            ("filename", file_name),
            ("fieldname", "file"),
            ("filetype", "application/zip"),
        ];

        // 5) Convert them into "key <base64-of-value>" lines, then join with commas
        let encoded_metadata: Vec<String> = metadata_fields
            .into_iter()
            .map(|(k, v)| format!("{} {}", k, BASE64.encode(v)))
            .collect();

        let upload_metadata = encoded_metadata.join(",");

        log_info!("upload_metadata {:?}", upload_metadata,);

        // === 1) Create the TUS file on the server (POST ...)
        let create_req = self
            .client
            .post(tus_url) // Transloadit’s TUS endpoint from assembly
            .header("Tus-Resumable", "1.0.0")
            .header("Upload-Length", file_size.to_string())
            .header("Upload-Metadata", upload_metadata)
            .send()
            .await
            .context("Failed TUS file creation (POST)")?;

        if !create_req.status().is_success() {
            let status = create_req.status();
            let body = create_req.text().await.ok();

            log_info!("TUS creation failed. Status: {}, Body: {:?}", status, body);

            return Err(anyhow!(
                "TUS creation failed. Status: {}, Body: {:?}",
                status,
                body
            ));
        }

        // TUS server responds with a `Location` header: the unique upload URL for this file
        let location_header = create_req
            .headers()
            .get("Location")
            .ok_or_else(|| anyhow!("No Location header from TUS create request"))?;

        let upload_url = location_header
            .to_str()
            .context("Cannot parse TUS Location header as string")?
            .to_owned();

        // === 2) PATCH the file data (the actual upload)
        // For large files, you might want to chunk this in a loop.
        // For smaller files, we can read all into memory or do a stream approach with partial patching.
        let file_data = fs::read(file_path)
            .await
            .context("Could not read archive to memory")?;

        let patch_resp = self
            .client
            .patch(&upload_url)
            .header("Tus-Resumable", "1.0.0")
            .header("Upload-Offset", 0.to_string())
            .header("Content-Type", "application/offset+octet-stream")
            .body(file_data)
            .send()
            .await
            .context("Failed TUS PATCH request")?;

        if !patch_resp.status().is_success() {
            return Err(anyhow!(
                "TUS upload patch failed. Status: {}, Body: {:?}",
                patch_resp.status(),
                patch_resp.text().await.ok()
            ));
        }

        Ok(())
    }
}
