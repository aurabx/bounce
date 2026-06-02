use crate::aura::aura_api::AuraApi;
use crate::db::database::{study_status, Database};
use crate::receiver::metadata::Metadata;
use crate::transmitter::backoff::{next_retry_delay, RetryPolicy};
use crate::{load_config, log_error, log_info};
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Cursor, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{collections::HashMap, sync::Arc};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncReadExt;
use tokio::sync::{oneshot, Mutex, Semaphore};
use tokio::time::{sleep, Duration};
use tokio::{fs, fs::File};
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};
use zip::result::ZipError;
use zip::{write::SimpleFileOptions, write::ZipWriter, CompressionMethod};
// use tokio_util::io::ReaderStream;
// use tokio_util::io::ReaderStream;

/// Maximum number of study uploads that may run concurrently across the whole
/// process. Both the debounce path and the retry scheduler acquire a permit
/// from the same global semaphore, so a burst of arriving studies (or a large
/// recovered backlog) cannot spawn an unbounded number of concurrent uploads.
const MAX_CONCURRENT_UPLOADS: usize = 2;

/// Process-global upload concurrency limiter. A `OnceLock` is used (rather than
/// a field on `Transmission`) so every `Transmission` instance — however it was
/// constructed — shares the same limiter.
static UPLOAD_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn upload_semaphore() -> Arc<Semaphore> {
    UPLOAD_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_UPLOADS)))
        .clone()
}

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
    database: Database,
}

impl Transmission {
    pub fn new(app_handle: AppHandle) -> Self {
        let database = app_handle.state::<Database>().inner().clone();
        Self {
            client: Client::new(),
            scheduled_studies: Arc::new(Mutex::new(HashMap::new())),
            aura_api: Arc::new(AuraApi::new(app_handle.clone())),
            app_handle,
            database,
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

                    if let Err(e) = self_clone.app_handle.emit("log", format!("Sending study {}", study_uid_clone)) {
                        log_error!("Failed to emit 'log' event: {}", e);
                    }

                    let study_uid = study_uid.clone();

                    // Route through attempt_upload so a failed send is recorded
                    // (status + attempt history) and becomes eligible for the
                    // retry scheduler, instead of being silently discarded.
                    self_clone.attempt_upload(study_uid).await;
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
        self.delete_local_study_meta(study_uid.clone()).await?;

        log_info!("Deleted local study files and archive for {}", study_uid);

        Ok(())
    }

    /// Shared database handle, used by the retry scheduler to query studies
    /// that are due for an upload attempt.
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Notify the UI that the upload-attempt audit trail has changed so the
    /// Transactions view can re-fetch its current page. Fire-and-forget: an
    /// emit failure during shutdown is logged but never aborts the upload.
    fn emit_transactions_updated(&self) {
        if let Err(e) = self.app_handle.emit("transactions-updated", ()) {
            log_error!("Failed to emit 'transactions-updated' event: {}", e);
        }
    }

    /// Build the retry policy from the current user configuration.
    fn retry_policy(&self) -> RetryPolicy {
        let config = load_config(self.app_handle.clone());
        RetryPolicy::from_config(
            config.retry_base_seconds,
            config.retry_cap_seconds,
            config.max_upload_attempts,
        )
    }

    /// Attempt a single upload of a study, recording the outcome.
    ///
    /// This is the only entry point that should drive an upload: it atomically
    /// claims the study (preventing concurrent double-uploads), bounds total
    /// concurrency via the global upload semaphore, runs [`Self::send_study`],
    /// and on failure records the attempt and schedules the next retry (or
    /// marks the study terminally `FAILED` once the retry budget is spent).
    ///
    /// Both the debounce path and the retry scheduler call this. Errors are
    /// handled and logged here rather than propagated, because callers are
    /// fire-and-forget background tasks.
    /// User-initiated send/retry of a study (the Send and Retry buttons, and
    /// bulk send). Re-queues the study so an already-`SENT` or `FAILED` study
    /// becomes eligible again, then runs a bounded upload attempt. Re-queuing
    /// skips studies currently `UPLOADING` so a manual click cannot start a
    /// duplicate concurrent upload.
    pub async fn manual_send(&self, study_uid: String) {
        if let Err(e) = self.database.reset_for_manual_retry(&study_uid).await {
            log_error!(
                "Failed to re-queue study {} for manual send: {}",
                study_uid,
                e
            );
            return;
        }
        self.attempt_upload(study_uid).await;
    }

    pub async fn attempt_upload(&self, study_uid: String) {
        // Bound total concurrent uploads process-wide. The permit is held for
        // the whole attempt and released on return regardless of outcome.
        let _permit = match upload_semaphore().acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                log_error!("Upload semaphore closed; dropping upload for {}", study_uid);
                return;
            }
        };

        // Refuse to start a new upload (which would create a large temporary
        // archive) when disk space is critically low. The study is left
        // QUEUED so the scheduler picks it up again once space is freed, and a
        // warning is surfaced to the UI.
        let config = load_config(self.app_handle.clone());
        if crate::store::disk::warn_if_low(&self.app_handle, &config) {
            let _ = self.database.reset_for_manual_retry(&study_uid).await;
            log_error!(
                "Skipping upload of {} due to critically low disk space",
                study_uid
            );
            return;
        }

        let upload_id = Uuid::new_v4();
        // A study may be picked up for upload from any of these states: freshly
        // received (debounce), queued/awaiting retry (scheduler), or a manual
        // retry which re-queues it.
        let eligible = [
            study_status::IN_PROGRESS,
            study_status::QUEUED,
            study_status::RETRYING,
        ];

        let (attempt_id, attempt_no) = match self
            .database
            .claim_study_for_upload(&study_uid, &eligible, &upload_id.to_string())
            .await
        {
            Ok(Some(claim)) => claim,
            Ok(None) => {
                log_info!(
                    "Study {} not claimable (already uploading or terminal); skipping",
                    study_uid
                );
                return;
            }
            Err(e) => {
                log_error!("Failed to claim study {} for upload: {}", study_uid, e);
                return;
            }
        };
        self.emit_transactions_updated();

        match self.send_study(study_uid.clone(), upload_id).await {
            Ok(()) => {
                if let Err(e) = self.database.mark_attempt_success(attempt_id).await {
                    log_error!("Failed to record upload success for {}: {}", study_uid, e);
                }
                self.emit_transactions_updated();
            }
            Err(err) => {
                let err_str = format!("{:#}", err);
                let policy = self.retry_policy();
                let next_retry_at = if policy.should_retry(attempt_no as u32) {
                    let delay = next_retry_delay(attempt_no as u32, &policy);
                    let delay = chrono::Duration::from_std(delay)
                        .unwrap_or_else(|_| chrono::Duration::seconds(60));
                    Some(Utc::now() + delay)
                } else {
                    None
                };

                if let Err(e) = self
                    .database
                    .mark_upload_failed(attempt_id, &study_uid, &err_str, next_retry_at)
                    .await
                {
                    log_error!("Failed to record upload failure for {}: {}", study_uid, e);
                }
                self.emit_transactions_updated();

                let outcome = if next_retry_at.is_some() {
                    "will retry"
                } else {
                    "giving up (FAILED)"
                };
                log_error!(
                    "Upload attempt {} for study {} failed ({}): {}",
                    attempt_no,
                    study_uid,
                    outcome,
                    err_str
                );
                let _ = self.app_handle.emit(
                    "log",
                    format!("Upload failed for {} ({}): {}", study_uid, outcome, err_str),
                );
            }
        }
    }

    pub async fn send_study(&self, study_uid: String, upload_id: Uuid) -> Result<()> {
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

        let require_str = |key: &str| -> Result<String> {
            upload_config
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| anyhow!("uploader config missing required string field '{}'", key))
        };

        let endpoint = require_str("endpoint")?;
        let bucket = require_str("bucket")?;
        let assembly_id = require_str("assembly_id")?;

        log_info!("endpoint: {}", endpoint.clone());
        log_info!("bucket: {}", bucket.to_string());

        // === Upload init
        self.aura_api
            .upload_init(
                study_uid.clone(),
                assembly_id.to_string(),
                upload_id.to_string(),
            )
            .await
            .context("Failed to send upload init to Aurabox")?;

        log_info!("Sent upload init to aura");

        if let Err(e) = self.app_handle.emit(
            "log",
            format!("Study data send to aurabox {}", study_uid.clone()),
        ) {
            log_error!("Failed to emit 'log' event: {}", e);
        }

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
            .context("Failed to send upload start to Aurabox")?;

        log_info!("Sent upload start to aura");

        // === Upload via TUS
        self.upload_via_tus(&upload_config, &archive_path, &upload_id)
            .await?;
        log_info!("Study sent successfully via TUS to {}", endpoint);

        if let Err(e) = self.app_handle.emit(
            "log",
            format!("Dicom send to aurabox storage {}", study_uid.clone()),
        ) {
            log_error!("Failed to emit 'log' event: {}", e);
        }

        // === Upload complete
        self.aura_api
            .upload_save(
                upload_id.clone().to_string(),
                assembly_id.to_string(),
                "complete",
            )
            .await
            .context("Failed to send upload complete to Aurabox")?;

        log_info!("Sent upload complete to aura");

        if let Err(e) = self.app_handle.emit(
            "log",
            format!("Complete request sent to aura {}", study_uid.clone()),
        ) {
            log_error!("Failed to emit 'log' event: {}", e);
        }

        // Clean up retrieve marker file if it exists (created by the
        // poller when a study is retrieved via C-MOVE).
        let retrieve_marker_path = config.resolve_retrieve_marker_path(&study_uid);
        if retrieve_marker_path.exists() {
            if let Err(e) = std::fs::remove_file(&retrieve_marker_path) {
                log_error!("Failed to delete retrieve marker: {}", e);
            }
        }

        // Optionally, delete local study if requested
        if delete_after_send {
            self.delete_study(study_uid.clone()).await?;
        }

        if let Err(err) =
            Metadata::update_study_metadata_status(&self.database, study_uid, study_status::SENT)
                .await
        {
            log_error!("Failed to update study metadata status: {}", err);
        }

        Ok(())
    }

    fn resolve_study_path(&self, study_uid: &str) -> PathBuf {
        let config = load_config(self.app_handle.clone());

        // Actually push the study (you’ll have to adapt to your code)
        let mut file_path = PathBuf::from(config.get_base_dir());
        file_path.push(study_uid.trim_end_matches('\0'));
        // file_path.push(study_uid.to_string());

        file_path
    }

    fn resolve_study_archive_path(&self, study_uid: &str) -> PathBuf {
        let config = load_config(self.app_handle.clone());

        let file_path = PathBuf::from(config.get_base_dir());
        file_path.join(study_uid.to_owned() + ".zip")
    }

    fn resolve_study_meta_path(&self, study_uid: &str) -> PathBuf {
        let config = load_config(self.app_handle.clone());

        let file_path = PathBuf::from(config.get_base_dir());
        file_path.join(study_uid.to_owned() + ".json")
    }

    pub async fn zip_folder<T, I>(it: I, prefix: &Path, writer: T) -> anyhow::Result<()>
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
        Self::zip_folder(it, study_path, file).await?;

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
        let mut entries = fs::read_dir(&file_path)
            .await
            .context(format!("Failed to read directory: {:?}", file_path))?;

        let mut deleted_count = 0;
        let mut error_count = 0;

        // Iterate through all entries in the directory
        while let Some(entry) = entries
            .next_entry()
            .await
            .context("Failed to read directory entry")?
        {
            let entry_path = entry.path();
            let entry_name = entry_path
                .file_name()
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
            log_error!(
                "Storage clearing completed with {} errors. {} items deleted.",
                error_count,
                deleted_count
            );
            return Err(anyhow!(
                "Failed to delete {} items from storage",
                error_count
            ));
        }

        log_info!(
            "Successfully cleared storage directory. {} items deleted.",
            deleted_count
        );
        Ok(())
    }

    /// Create a Transloadit Assembly and return its TUS upload URL.
    async fn fetch_uploader_config(&self) -> Result<Value> {
        let upload_config = self.aura_api.upload_config().await?;

        let lift_config = upload_config
            .get("lift")
            .ok_or_else(|| anyhow!("uploader config missing 'lift' object"))?;

        let require_str = |obj: &Value, key: &str| -> Result<String> {
            obj.get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| anyhow!("uploader config missing required string field '{}'", key))
        };

        let bucket = require_str(lift_config, "bucket")?;
        let endpoint = require_str(lift_config, "endpoint")?;
        let token = require_str(lift_config, "token")?;
        let assembly_id = require_str(lift_config, "assembly_id")?;
        let upload_type = require_str(&upload_config, "type")?;
        let mode = require_str(&upload_config, "mode")?;

        log_info!("Uploader config bucket: {:#?}", bucket);
        log_info!("Uploader config endpoint: {:#?}", endpoint);

        let resp_json = json!({
            "bucket": bucket,
            "endpoint": endpoint,
            "token": token,
            "assembly_id": assembly_id,
            "type": upload_type,
            "mode": mode,
        });

        Ok(resp_json)
    }

    /// A simple TUS upload example.
    async fn upload_via_tus(
        &self,
        upload_config: &Value,
        file_path: &Path,
        upload_id: &Uuid,
    ) -> Result<()> {
        let file_size = fs::metadata(file_path)
            .await
            .context("Could not get metadata for file")?
            .len();

        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.dcm.zip");

        let require_str = |key: &str| -> Result<&str> {
            upload_config
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("uploader config missing required string field '{}'", key))
        };

        let endpoint = require_str("endpoint")?;
        let token = require_str("token")?;
        let mode = require_str("mode")?;
        let bucket = require_str("bucket")?;
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

        log_info!(
            "Starting chunked upload of {} bytes in {}MB chunks",
            file_size,
            CHUNK_SIZE / 1024 / 1024
        );

        loop {
            // Read next chunk using AsyncReadExt::read on the cursor
            let bytes_read = cursor
                .read(&mut chunk_buffer)
                .await
                .context("Failed to read file chunk")?;

            if bytes_read == 0 {
                break; // EOF reached
            }

            // Upload this chunk
            let chunk_data = &chunk_buffer[..bytes_read];

            log_info!(
                "Uploading chunk: offset={}, size={}",
                uploaded_bytes,
                bytes_read
            );

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
                log_error!(
                    "TUS chunk upload failed. Status: {}, Body: {:?}",
                    status,
                    body
                );
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
                log_info!(
                    "Upload progress: {}% ({}/{})",
                    progress,
                    uploaded_bytes,
                    file_size
                );

                if let Err(e) = self.app_handle.emit(
                    "upload-progress",
                    json!({
                        "uploaded": uploaded_bytes,
                        "total": file_size,
                        "progress": progress
                    }),
                ) {
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
