#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod aura;
mod db;
mod dimse;
mod logger;
mod query;
mod receiver;
mod send;
mod store;
mod transmitter;

use crate::aura::aura_api::AuraApi;
use crate::db::database::Database;
use crate::logger::{
    init_logtail_channel, logtail_dispatch, set_remote_logging_enabled, start_logtail_sender,
    LogtailConfig,
};
use crate::query::cecho::execute_cecho;
use crate::query::cfind::execute_cfind;
use crate::query::models::{CfindResult, DicomService, PacsService, QueryFilters};
use crate::receiver::server::init_server_state;
use crate::store::pacs_cache::{load_cached_services, save_cached_services};
use crate::transmitter::transmission::{QueueUpload, Transmission};
use std::sync::Arc;
use store::config::Config;
use tauri::{AppHandle, Emitter, Listener, Manager, WindowEvent};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    pub transmission: Transmission,
}

fn load_config(app: AppHandle) -> Config {
    Config::load(app)
}

#[tauri::command]
fn send_log(app: AppHandle, log: String) -> Result<(), String> {
    println!("log: {}", log);
    app.emit("log", log).unwrap();

    Ok(())
}

#[tauri::command]
fn update_send_logs(enabled: bool) -> Result<(), String> {
    set_remote_logging_enabled(enabled);
    Ok(())
}

/// Verify that the currently saved API key can authenticate against the
/// Aurabox API. Performs a GET against the bounce uploader config endpoint
/// and returns the resolved endpoint URL on success.
///
/// Returns an error string suitable for direct display in the UI when the
/// request fails (network error, 401/403, malformed key, etc.).
#[tauri::command]
async fn verify_connectivity(app: AppHandle) -> Result<String, String> {
    let config = load_config(app.clone());

    if config.api_key.trim().is_empty() {
        return Err("API key is not set.".to_string());
    }

    if config.region_from_api_key().is_none() {
        return Err("API key is malformed (expected aura_<region>_bounce_…).".to_string());
    }

    let endpoint = config.get_api_endpoint();
    let aura_api = AuraApi::new(app);

    aura_api
        .upload_config()
        .await
        .map(|_| endpoint)
        .map_err(|e| format!("Could not reach {}: {}", config.get_api_endpoint(), e))
}

#[tauri::command]
async fn reset_app(app: AppHandle) -> Result<(), String> {
    let database = app.state::<Database>();

    database.clear_studies().await.expect("clear studies panic");

    let transmission = Transmission::new(app);

    transmission
        .clear_storage()
        .await
        .expect("clear study panic");

    Ok(())
}

#[tauri::command]
async fn api_start_upload(
    app: AppHandle,
    study_uid: String,
    signature: String,
    upload_id: String,
    assembly_id: String,
) -> Result<(), String> {
    let aura_api = AuraApi::new(app);

    aura_api
        .upload_init(study_uid, signature, upload_id.clone())
        .await
        .expect("api start upload panic");

    aura_api
        .upload_save(upload_id.clone(), assembly_id.clone(), "update")
        .await
        .expect("Error sending upload update api message");

    Ok(())
}

#[tauri::command]
async fn send_study(app: AppHandle, study_uid: String) -> Result<(), String> {
    let transmission = Transmission::new(app);

    // manual_send re-queues then runs a bounded, failure-recording attempt;
    // a failed send is now retried automatically rather than panicking.
    transmission.manual_send(study_uid).await;

    Ok(())
}

#[tauri::command]
async fn retry_study(app: AppHandle, study_uid: String) -> Result<(), String> {
    let transmission = Transmission::new(app);

    // Same path as a manual send: re-queue the (typically FAILED) study and
    // attempt the upload immediately.
    transmission.manual_send(study_uid).await;

    Ok(())
}

#[tauri::command]
async fn study_upload_attempts(
    app: AppHandle,
    study_uid: String,
) -> Result<serde_json::Value, String> {
    let database = app.state::<Database>();
    let attempts = database
        .upload_attempts_for_study(&study_uid)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(attempts).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_study(app: AppHandle, study_uid: String) -> Result<(), String> {
    let transmission = Transmission::new(app.clone());
    let database = app.state::<Database>();

    transmission
        .delete_study(study_uid.clone())
        .await
        .expect("delete study panic");

    transmission
        .delete_local_study_meta(study_uid.clone())
        .await
        .expect("delete study meta panic");

    database
        .delete_study(study_uid.clone())
        .await
        .expect("delete study meta panic");

    Ok(())
}

#[tauri::command]
async fn receiver_start(app: AppHandle) -> Result<(), String> {
    println!("receiver_start: now");

    app.emit("log", "Starting server").unwrap();
    log_info!("Starting server");

    receiver::server::start(app)
        .await
        .map_err(|e| format!("Failed to start server: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn receiver_stop(app: AppHandle) -> Result<(), String> {
    app.emit("log", "Stopping server").unwrap();

    receiver::server::stop(app)
        .await
        .map_err(|e| format!("Failed to stop server: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn current_studies(
    app: AppHandle,
    page: Option<u32>,
    limit: Option<u32>,
    search: Option<String>,
) -> Result<(), String> {
    let page = page.unwrap_or(1);
    let limit = limit.unwrap_or(10);
    let database = app.state::<Database>();

    let studies = database.current_studies(page, limit, search).await;

    app.emit("current-studies", studies).unwrap();
    Ok(())
}

/// Best-effort bulk send: iterate over the supplied Study UIDs and call
/// `Transmission::send_study` on each. Per-item failures are logged but
/// do not abort the batch — medical-imaging operators expect every
/// selected item to be attempted.
#[tauri::command]
async fn bulk_send_studies(app: AppHandle, study_uids: Vec<String>) -> Result<(), String> {
    let transmission = Transmission::new(app);
    for uid in study_uids {
        // Each manual send re-queues and attempts the upload; failures are
        // recorded and retried automatically rather than aborting the batch.
        transmission.manual_send(uid).await;
    }
    Ok(())
}

/// Best-effort bulk delete: removes the on-disk study, the local meta,
/// and the database row for each UID. Failures are logged per item and
/// the batch continues so a single corrupt entry does not block
/// removing the rest.
#[tauri::command]
async fn bulk_delete_studies(app: AppHandle, study_uids: Vec<String>) -> Result<(), String> {
    let transmission = Transmission::new(app.clone());
    let database = app.state::<Database>();
    for uid in study_uids {
        if let Err(e) = transmission.delete_study(uid.clone()).await {
            log_error!("bulk_delete_studies: delete_study failed for {}: {}", uid, e);
        }
        if let Err(e) = transmission.delete_local_study_meta(uid.clone()).await {
            log_error!(
                "bulk_delete_studies: delete_local_study_meta failed for {}: {}",
                uid,
                e
            );
        }
        if let Err(e) = database.delete_study(uid.clone()).await {
            log_error!("bulk_delete_studies: db delete failed for {}: {}", uid, e);
        }
    }
    Ok(())
}

/// Clear every study from the database and remove the on-disk storage
/// directory. Distinct from `reset_app` so that future widening of
/// reset behaviour (settings, credentials) does not silently widen
/// what the Studies tab "Delete all" button does.
#[tauri::command]
async fn delete_all_studies(app: AppHandle) -> Result<(), String> {
    let database = app.state::<Database>();
    let count = database
        .clear_studies()
        .await
        .map_err(|e| e.to_string())?;

    let transmission = Transmission::new(app);
    transmission
        .clear_storage()
        .await
        .map_err(|e| e.to_string())?;

    log_info!("delete_all_studies: removed {} studies", count);
    Ok(())
}

/// Diagnostic command: execute a C-FIND query directly from the Bounce UI.
///
/// This is independent of the Aurabox polling flow and useful for testing
/// connectivity to a PACS during setup.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn cfind_query(
    app: AppHandle,
    pacs_host: String,
    pacs_port: u16,
    pacs_ae_title: String,
    patient_name: Option<String>,
    patient_id: Option<String>,
    study_date: Option<String>,
    accession_number: Option<String>,
    modality: Option<String>,
) -> Result<Vec<CfindResult>, String> {
    let config = load_config(app);

    let pacs = PacsService {
        ae_title: pacs_ae_title,
        host: pacs_host,
        port: pacs_port,
    };

    let filters = QueryFilters {
        patient_name,
        patient_id,
        study_date,
        accession_number,
        modality,
        study_instance_uid: None,
    };

    execute_cfind(&config.ae_title, &pacs, "STUDY", &filters).await
}

/// Return the locally cached PACS services list. Reads synchronously from
/// the Tauri plugin-store so the PACS tab can render immediately on mount,
/// even when Aurabox is unreachable. Empty if no refresh has yet succeeded.
#[tauri::command]
fn list_pacs_services(app: AppHandle) -> Result<Vec<DicomService>, String> {
    Ok(load_cached_services(&app))
}

/// Fetch the configured PACS services list from Aurabox, persist the
/// response into the local cache, and return the fresh list. The Refresh
/// button on the PACS tab and the startup probe both call this; the cache
/// is only updated on a successful round trip so a transient failure does
/// not blank the UI.
#[tauri::command]
async fn refresh_pacs_services(app: AppHandle) -> Result<Vec<DicomService>, String> {
    let api = AuraApi::new(app.clone());
    let response = api
        .fetch_services()
        .await
        .map_err(|e| format!("Failed to fetch PACS services: {}", e))?;

    save_cached_services(&app, &response.services)?;

    Ok(response.services)
}

/// Result of a single C-ECHO attempt against a configured PACS.
///
/// `latency_ms` is wall-clock from the initial association request to the
/// C-ECHO-RSP arriving — it includes association setup, not just the round
/// trip of the C-ECHO PDU itself.
#[derive(serde::Serialize)]
struct EchoResult {
    ok: bool,
    latency_ms: Option<u64>,
    error: Option<String>,
}

/// Run a C-ECHO against the cached PACS service identified by `service_id`.
///
/// Looks the service up in the local cache rather than re-fetching from
/// Aurabox so the click-to-echo path is fast and works offline. Returns a
/// shaped result rather than `Result<_, String>` so the UI can render
/// success and failure consistently inline next to the row.
#[tauri::command]
async fn echo_pacs_service(app: AppHandle, service_id: String) -> Result<EchoResult, String> {
    let services = load_cached_services(&app);
    let service = services
        .into_iter()
        .find(|s| s.id == service_id)
        .ok_or_else(|| format!("PACS service {} not in cache; refresh and try again", service_id))?;

    let config = load_config(app);
    let pacs: PacsService = service.into();

    match execute_cecho(&config.ae_title, &pacs).await {
        Ok(report) => Ok(EchoResult {
            ok: true,
            latency_ms: Some(report.latency_ms),
            error: None,
        }),
        Err(e) => Ok(EchoResult {
            ok: false,
            latency_ms: None,
            error: Some(e),
        }),
    }
}

#[tauri::command]
fn show_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    // Construct the Logtail fan-out channel BEFORE the Tauri builder so the
    // custom target registered on tauri-plugin-log has a live sender during
    // plugin initialization. The background HTTP shipper is started later
    // inside `.setup()` once the API key is loaded from the config store.
    init_logtail_channel();

    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Registers Bounce with the OS login-items mechanism (LaunchAgent on
        // macOS, registry Run key on Windows) so the receiver comes back up
        // after a reboot. The toggle in Settings drives enable()/disable().
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .setup(|app| {
            let handle = app.app_handle();
            let config = Config::load(handle.clone());

            // Initialize database
            let db_handle = handle.clone();
            tauri::async_runtime::block_on(async move {
                match Database::new(db_handle).await {
                    Ok(database) => {
                        handle.manage(database);
                        log_info!("Database initialized successfully");
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize database: {}", e);
                        std::process::exit(1);
                    }
                }
            });

            let logtail_config = LogtailConfig {
                enable: config.send_logs == "yes",
                source_token: "rnMSXo4FKewJaKkoLaYjRsvF".to_string(),
                endpoint: "https://s1489595.ap-sng-8.betterstackdata.com".to_string(),
                app_name: config.api_key,
                hostname: gethostname::gethostname().to_string_lossy().to_string(),
            };

            start_logtail_sender(logtail_config);

            let transmission = Transmission::new(handle.clone());

            // store it in Tauri's managed state
            app.manage(AppState { transmission });

            let app_handle = app.handle().clone();

            app.listen("queue-study", move |event| {
                // Clone the payload to a String first
                let payload_str = event.payload().to_string();

                // Process outside of the event handler
                let app_handle_clone = app_handle.clone();

                tauri::async_runtime::spawn(async move {
                    if let Ok(payload) = serde_json::from_str::<QueueUpload>(&payload_str) {
                        log_info!("queue-study {}", payload.study_uid);
                        let study_uid = payload.study_uid;

                        // Get the manager inside the async block
                        let transmission =
                            app_handle_clone.state::<AppState>().transmission.clone();

                        transmission
                            .schedule_study_push(study_uid.to_string())
                            .await
                            .expect("Enable to schedule study push");
                    }
                });
            });

            // Initialize and manage server state
            app.manage(Arc::new(Mutex::new(init_server_state())));

            // Refresh the PACS services cache from Aurabox in the background
            // so the PACS tab shows fresh data on first open. Skipped if the
            // API key is unset; failures are logged and leave the cache as-is.
            let pacs_refresh_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let cfg = Config::load(pacs_refresh_handle.clone());
                if cfg.api_key.trim().is_empty() {
                    return;
                }

                let api = AuraApi::new(pacs_refresh_handle.clone());
                match api.fetch_services().await {
                    Ok(resp) => {
                        if let Err(e) =
                            save_cached_services(&pacs_refresh_handle, &resp.services)
                        {
                            log_error!("Startup PACS cache refresh: save failed: {}", e);
                        } else {
                            log_info!(
                                "Startup PACS cache refresh: cached {} services",
                                resp.services.len()
                            );
                        }
                    }
                    Err(e) => {
                        log_error!("Startup PACS cache refresh: fetch failed: {}", e);
                    }
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(Target::new(TargetKind::LogDir {
                    file_name: Some("logs".to_string()),
                }))
                .target(Target::new(TargetKind::Stdout))
                .target(Target::new(TargetKind::Webview))
                // Fan-out target: forwards records into the Logtail mpsc
                // channel. The background shipper started in `.setup()` drains
                // it and POSTs batches to Better Stack.
                .target(Target::new(TargetKind::Dispatch(logtail_dispatch())))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Don't close the window, just hide it
                window.hide().unwrap();
                // Prevent the window from actually closing
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            receiver_start,
            receiver_stop,
            reset_app,
            send_log,
            send_study,
            retry_study,
            study_upload_attempts,
            delete_study,
            bulk_send_studies,
            bulk_delete_studies,
            delete_all_studies,
            api_start_upload,
            current_studies,
            cfind_query,
            list_pacs_services,
            refresh_pacs_services,
            echo_pacs_service,
            show_window,
            update_send_logs,
            verify_connectivity
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
