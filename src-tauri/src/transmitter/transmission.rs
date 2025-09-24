use crate::aura::aura_api::AuraApi;
use crate::{load_config, log_error, log_info};
use tokio::io::AsyncReadExt;
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::{multipart, Client};
use serde_json::{json, Value};
use std::io::{Seek, Write, Cursor};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, sync::Arc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use tokio::time::{sleep, Duration};
use tokio::{fs, fs::File};
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};
use zip::result::ZipError;
use zip::{write::SimpleFileOptions, write::ZipWriter, CompressionMethod};
use crate::receiver::metadata::Metadata;
// use tokio_util::io::ReaderStream;

#[derive(Debug)]
pub struct ScheduledStudy {
    // last_received: Instant,
    // We send a signal to the task whenever we want to reset the countdown.
    cancel_tx: oneshot::Sender<()>,
}


#[derive(Clone, Serialize, Deserialize)]
pub struct QueueUpload<'a> {
    pub study_uid: &'a str,
}

#[derive(Debug, Clone)]
pub struct Transmission {
    client: Client,
    scheduled_studies: Arc<Mutex<HashMap<String, ScheduledStudy>>>,
    aura_api: Arc<AuraApi>,
    app_handle: AppHandle,
}

impl Transmission {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            client: Client::new(),
            scheduled_studies: Arc::new(Mutex::new(HashMap::new())),
            aura_api: Arc::new(AuraApi::new(app_handle.clone())),
            app_handle
        }
    }

    /// Schedule pushing the study in 10 seconds—debouncing repeated calls.
    /// If another call comes in for the same study before the 10s ends,
    /// we cancel and restart the countdown.
    pub async fn schedule_study_push(&self, study_uid: String) -> anyhow::Result<()> {
        log_info!("Scheduling study push for {}", &study_uid);

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

                    self_clone.app_handle.emit("log", format!("Sending study {}", study_uid_clone)).unwrap();

                    // do the actual push logic here, e.g. `self_clone.send_study(...).await`
                    let study_uid = study_uid.clone();

                    // tauri::async_runtime::spawn(async move {
                    //     sleep(Duration::from_secs(30)).await;
                    //     log_info!("moved run {}", study_uid_clone);
                    // });

                    let _ = self_clone.send_study(study_uid).await;
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
        self.delete_local_compressed_study(study_uid.clone()).await?;
        self.delete_local_study_meta(study_uid.clone()).await?;

        log_info!("Deleted local study files and archive for {}", study_uid);

        Ok(())
    }

    pub async fn send_study(&self, study_uid: String) -> Result<()> {
        let upload_id = Uuid::new_v4();

        let config = load_config(self.app_handle.clone());
        let delete_after_send = config.delete_after_success == "yes";

        log_info!(
            "Starting send_study {} for upload: {}",
            &study_uid,
            &upload_id
        );

        let study_path = self.resolve_study_path(&study_uid);

        log_info!("Preparing to send study: {:?}", &study_path);

        let archive_path = self.compress_study(study_uid.clone()).await?;
        log_info!("Preparing to send study zip: {:?}", archive_path);

        // === Create an Assembly on Transloadit, get the TUS URL back
        // let assembly = self.create_transloadit_assembly(&upload_id).await?;

        // === Get upload config from aura
        let upload_config = self.fetch_uploader_config().await?;

        log_info!("study_uid: {}", study_uid.clone());
        log_info!("upload_id: {}", upload_id.to_string());

        let endpoint = upload_config.get("endpoint").unwrap().as_str().unwrap().to_string();
        let token = upload_config.get("token").unwrap().as_str().unwrap().to_string();
        let bucket = upload_config.get("bucket").unwrap().as_str().unwrap().to_string();
        let assembly_id = upload_config.get("assembly_id").unwrap().as_str().unwrap().to_string();

        log_info!("endpoint: {}", endpoint.clone());
        log_info!("token: {}", token.to_string());
        log_info!("bucket: {}", bucket.to_string());


        // === Upload init
        self.aura_api
            .upload_init(
                study_uid.clone(),
                assembly_id.to_string(),
                upload_id.to_string(),
            )
            .await
            .expect("Error sending upload init api message");

        log_info!("Sent upload init to aura");

        self.app_handle.emit("log", format!("Study data send to aurabox {}", study_uid.clone())).unwrap();

        // === Upload start
        // This would normally run just after the upload starts in uppy
        // so we run it here before upload starts.
        self.aura_api
            .upload_save(
                upload_id.clone().to_string(),
                assembly_id.to_string(),
                "start",
            )
            .await
            .expect("Error sending upload start api message");

        log_info!("Sent upload start to aura");

        // === Upload via TUS
        self.upload_via_tus(&upload_config, &archive_path, &upload_id).await?;
        log_info!(
            "Study sent successfully via TUS to {}",
            endpoint
        );

        self.app_handle.emit("log", format!("Dicom send to aurabox storage {}", study_uid.clone())).unwrap();

        // === Upload complete
        self.aura_api
            .upload_save(
                upload_id.clone().to_string(),
                assembly_id.to_string(),
                "complete",
            )
            .await
            .expect("Error sending complete api message");

        log_info!("Sent upload complete to aura");

        self.app_handle.emit("log", format!("Complete request sent to aura {}", study_uid.clone())).unwrap();

        // Optionally, delete local study if requested
        if delete_after_send {
            self.delete_study(study_uid.clone()).await?;
        }

        if let Err(err) = Metadata::update_study_metadata_status(
            &self.app_handle,
            study_uid,
            "SENT",
        ).await {
            log_error!("Failed to update study metadata status: {}", err);
        }

        Ok(())
    }

    fn resolve_study_path(&self, study_uid: &String) -> PathBuf {
        let config = load_config(self.app_handle.clone());

        // Actually push the study (you’ll have to adapt to your code)
        let mut file_path = PathBuf::from(config.get_base_dir());
        file_path.push(study_uid.trim_end_matches('\0').to_string());
        // file_path.push(study_uid.to_string());

        file_path
    }

    fn resolve_study_archive_path(&self, study_uid: &String) -> PathBuf {
        let config = load_config(self.app_handle.clone());

        let file_path = PathBuf::from(config.get_base_dir());
        let file_path = file_path.clone().join(study_uid.clone() + ".zip");

        file_path
    }

    fn resolve_study_meta_path(&self, study_uid: &String) -> PathBuf {
        let config = load_config(self.app_handle.clone());

        let file_path = PathBuf::from(config.get_base_dir());
        let file_path = file_path.clone().join(study_uid.clone() + ".json");

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

    async fn compress_study(&self, study_uid: String) -> Result<PathBuf> {

        //let archive_path = study_path.with_extension("zip");
        let config = load_config(self.app_handle.clone());
        let output_path = PathBuf::from(config.get_base_dir());
        let archive_path = output_path.clone().join(study_uid.clone() + ".zip");
        log_info!("Compressing with_extension {archive_path:?}");

        let study_path_buf = output_path.join(study_uid);
        log_info!("compressing study_path {study_path_buf:?}");

        let study_path = study_path_buf.as_path();

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

        if study_path.exists() {
            fs::remove_dir_all(study_path.as_path())
                .await
                .context("Failed to delete local study files")?;
        }

        Ok(())
    }

    async fn delete_local_compressed_study(&self, study_uid: String) -> Result<()> {
        let archive_path = self.resolve_study_archive_path(&study_uid);

        if archive_path.exists() {
            fs::remove_file(archive_path.as_path())
                .await
                .context("Failed to delete local study files")?;

        }

        Ok(())
    }

    pub async fn delete_local_study_meta(&self, study_uid: String) -> Result<()> {
        let meta_path = self.resolve_study_meta_path(&study_uid);

        if meta_path.exists() {
            fs::remove_file(meta_path.as_path())
                .await
                .context("Failed to delete local study meta file")?;
        }

        Ok(())
    }

    pub async fn clear_storage(&self) -> Result<()> {
        let config = load_config(self.app_handle.clone());

        let file_path = PathBuf::from(config.get_base_dir());

        // Check if the directory exists
        if !file_path.exists() {
            log_info!("Storage directory does not exist: {:?}", file_path);
            return Ok(());
        }

        if !file_path.is_dir() {
            return Err(anyhow!("Storage path is not a directory: {:?}", file_path));
        }

        // Read directory contents
        let mut entries = fs::read_dir(&file_path).await
            .context(format!("Failed to read directory: {:?}", file_path))?;

        let mut deleted_count = 0;
        let mut error_count = 0;

        // Iterate through all entries in the directory
        while let Some(entry) = entries.next_entry().await
            .context("Failed to read directory entry")? {

            let entry_path = entry.path();
            let entry_name = entry_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown");

            if entry_path.is_dir() {
                // Remove directory and all its contents
                match fs::remove_dir_all(&entry_path).await {
                    Ok(_) => {
                        log_info!("Deleted directory: {}", entry_name);
                        deleted_count += 1;
                    }
                    Err(e) => {
                        log_error!("Failed to delete directory {}: {}", entry_name, e);
                        error_count += 1;
                    }
                }
            } else if entry_path.is_file() {
                // Remove file
                match fs::remove_file(&entry_path).await {
                    Ok(_) => {
                        log_info!("Deleted file: {}", entry_name);
                        deleted_count += 1;
                    }
                    Err(e) => {
                        log_error!("Failed to delete file {}: {}", entry_name, e);
                        error_count += 1;
                    }
                }
            }
        }

        if error_count > 0 {
            log_error!("Storage clearing completed with {} errors. {} items deleted.", error_count, deleted_count);
            return Err(anyhow!("Failed to delete {} items from storage", error_count));
        }

        log_info!("Successfully cleared storage directory. {} items deleted.", deleted_count);
        Ok(())
    }


    /// Create a Transloadit Assembly and return its TUS upload URL.
    async fn fetch_uploader_config(&self) -> Result<Value> {
        let upload_config = self.aura_api.upload_config().await?;

        let lift_config = upload_config
            .get("lift")
            .unwrap();

        let bucket = lift_config.get("bucket").unwrap().as_str();
        let endpoint = lift_config.get("endpoint").unwrap().as_str();

        log_info!("Uploader config bucket: {:#?}", bucket);
        log_info!("Uploader config endpoint: {:#?}", endpoint);

        let resp_json = json!({
            "bucket": bucket,
            "endpoint": endpoint,
            "token": lift_config.get("token").unwrap().as_str(),
            "assembly_id": lift_config.get("assembly_id").unwrap().as_str(),
            "type": upload_config.get("type").unwrap().as_str(),
            "mode": upload_config.get("mode").unwrap().as_str(),
        });

        Ok(resp_json)
    }

    /// A simple TUS upload example.
    async fn upload_via_tus(&self, upload_config: &Value, file_path: &Path, upload_id: &Uuid) -> Result<()> {
        let file_size = fs::metadata(file_path)
            .await
            .context("Could not get metadata for file")?
            .len();

        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.dcm.zip");

        let endpoint = upload_config.get("endpoint").unwrap().as_str().unwrap();
        let token = upload_config.get("token").unwrap().as_str().unwrap();
        let mode = upload_config.get("mode").unwrap().as_str().unwrap();
        let bucket = upload_config.get("bucket").unwrap().as_str().unwrap();
        let upload_id = upload_id.to_string();

        let metadata_fields = vec![
            ("name", file_name),
            ("type", "application/zip"),
            ("filename", file_name),
            ("fieldname", "file"),
            ("bucket", bucket),
            ("mode", mode),
            ("upload_id", upload_id.as_str()),
            ("filetype", "application/zip"),
        ];

        let encoded_metadata: Vec<String> = metadata_fields
            .into_iter()
            .map(|(k, v)| format!("{} {}", k, BASE64.encode(v)))
            .collect();

        let upload_metadata = encoded_metadata.join(",");

        log_info!("upload_metadata {:?}", upload_metadata,);

        // === 1) Create the TUS file on the server (POST ...)
        let create_req = self
            .client
            .post(endpoint)
            .header("Tus-Resumable", "1.0.0")
            .header("Upload-Length", file_size.to_string())
            .header("Upload-Metadata", upload_metadata.clone())
            .header("Content-Length", "0")
            .header("Authorization", format!("Bearer {}", token))
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

        let location_header = create_req
            .headers()
            .get("Location")
            .ok_or_else(|| anyhow!("No Location header from TUS create request"))?;

        let upload_url = location_header
            .to_str()
            .context("Cannot parse TUS Location header as string")?
            .to_owned();

        // === 2) PATCH the file data (the actual upload)
        const CHUNK_SIZE: usize = 1024 * 1024 * 5; // 5MB chunks

        // Read the entire file into memory and create a cursor
        let file_data = fs::read(file_path)
            .await
            .context("Could not read archive to memory")?;

        let mut cursor = Cursor::new(file_data);
        let mut uploaded_bytes = 0u64;
        let mut chunk_buffer = vec![0u8; CHUNK_SIZE];

        log_info!("Starting chunked upload of {} bytes in {}MB chunks", file_size, CHUNK_SIZE / 1024 / 1024);

        loop {
            // Read next chunk using AsyncReadExt::read on the cursor
            let bytes_read = cursor.read(&mut chunk_buffer)
                .await
                .context("Failed to read file chunk")?;

            if bytes_read == 0 {
                break; // EOF reached
            }

            // Upload this chunk
            let chunk_data = &chunk_buffer[..bytes_read];

            log_info!("Uploading chunk: offset={}, size={}", uploaded_bytes, bytes_read);

            let patch_resp = self
                .client
                .patch(&upload_url)
                .header("Tus-Resumable", "1.0.0")
                .header("Upload-Offset", uploaded_bytes.to_string())
                .header("Content-Type", "application/offset+octet-stream")
                .header("Upload-Metadata", upload_metadata.clone())
                .header("Authorization", format!("Bearer {}", token))
                .body(chunk_data.to_vec())
                .send()
                .await
                .context("Failed TUS PATCH request")?;

            if !patch_resp.status().is_success() {
                let status = patch_resp.status();
                let body = patch_resp.text().await.ok();
                log_error!("TUS chunk upload failed. Status: {}, Body: {:?}", status, body);
                return Err(anyhow!(
                    "TUS upload patch failed. Status: {}, Body: {:?}",
                    status,
                    body
                ));
            }

            uploaded_bytes += bytes_read as u64;

            // Report progress
            let progress = (uploaded_bytes as f64 / file_size as f64 * 100.0) as u32;
            if uploaded_bytes % (CHUNK_SIZE as u64 * 10) == 0 || uploaded_bytes == file_size {
                log_info!("Upload progress: {}% ({}/{})", progress, uploaded_bytes, file_size);

                if let Err(e) = self.app_handle.emit("upload-progress", json!({
                    "uploaded": uploaded_bytes,
                    "total": file_size,
                    "progress": progress
                })) {
                    log_error!("Failed to emit progress event: {}", e);
                }
            }
        }

        log_info!("Upload completed successfully: {} bytes", uploaded_bytes);

        if uploaded_bytes != file_size {
            return Err(anyhow!(
                "Upload incomplete: expected {} bytes, uploaded {} bytes",
                file_size,
                uploaded_bytes
            ));
        }

        Ok(())
    }
}
