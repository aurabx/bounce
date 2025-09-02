#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod aura;
mod logger;
mod receiver;
mod store;
mod transmitter;
mod db;

use crate::aura::aura_api::AuraApi;
use crate::receiver::server::init_server_state;
use crate::transmitter::transmission::{QueueUpload, Transmission};
use std::sync::Arc;
use store::config::Config;
use tauri::{AppHandle, Emitter, Listener, Manager, WindowEvent};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;
use crate::db::database::Database;
use crate::logger::{setup_logger_with_config, LogtailConfig};

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
async fn api_start_upload(
    app: AppHandle,
    study_uid: String,
    signature: String,
    upload_id: String,
    assembly_id: String,
) -> Result<(), String> {
    let aura_api = AuraApi::new(app);

    aura_api
        .upload_start(study_uid, signature, upload_id.clone())
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

    transmission
        .send_study(study_uid)
        .await
        .expect("send study panic");

    Ok(())
}

#[tauri::command]
async fn delete_study(app: AppHandle, study_uid: String) -> Result<(), String> {
    let transmission = Transmission::new(app);
    let database = app.state::<Database>();

    transmission
        .delete_study(study_uid.clone())
        .await
        .expect("delete study panic");

    transmission
        .delete_local_study_meta(study_uid.clone())
        .await
        .expect("delete study meta panic");

    database.delete_study(study_uid.clone())
        .await
        .expect("delete study meta panic");

    Ok(())
}

#[tauri::command]
async fn receiver_start(app: AppHandle) -> Result<(), String> {
    println!("receiver_start: {}", "now");

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
async fn current_studies(app: AppHandle) -> Result<(), String> {
    // let config = load_config(app.clone());
    let database = app.state::<Database>();

    let studies = database.current_studies().await;

    app.emit("current-studies", studies)
        .unwrap();
    Ok(())
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
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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

            setup_logger_with_config(logtail_config);

            let transmission = Transmission::new(handle.clone());

            // store it in Tauri's managed state
            app.manage(AppState {
                transmission
            });

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
                        let transmission = app_handle_clone.state::<AppState>().transmission.clone();

                        transmission.schedule_study_push(study_uid.to_string())
                            .await.expect("Enable to schedule study push");
                    }
                });
            });

            // Initialize and manage server state
            app.manage(Arc::new(Mutex::new(init_server_state())));

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
                .build(),
        )
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                // Don't close the window, just hide it
                window.hide().unwrap();
                // Prevent the window from actually closing
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            receiver_start,
            receiver_stop,
            send_log,
            send_study,
            delete_study,
            api_start_upload,
            current_studies,
            show_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
