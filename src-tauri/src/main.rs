#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod aura;
mod logger;
mod receiver;
mod store;
mod transmitter;

use crate::aura::aura_api::AuraApi;
use crate::logger::setup_logger;
use crate::receiver::server::init_server_state;
use crate::transmitter::manager::TransmissionManager;
use crate::transmitter::transmission::Transmission;
use std::sync::Arc;
use store::config::Config;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    tx_manager: TransmissionManager,
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
        .send_study(study_uid, false)
        .await
        .expect("send study panic");

    Ok(())
}

#[tauri::command]
async fn delete_study(app: AppHandle, study_uid: String) -> Result<(), String> {
    let transmission = Transmission::new(app);

    transmission
        .delete_study(study_uid)
        .await
        .expect("delete study panic");

    Ok(())
}

#[tauri::command]
async fn receiver_start(app: AppHandle) -> Result<(), String> {
    println!("receiver_start: {}", "now");

    app.emit("log", "Starting server").unwrap();

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
fn current_studies(app: AppHandle) -> Result<(), String> {
    let config = load_config(app.clone());
    app.emit("current-studies", config.current_studies())
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
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            // Setup logging
            setup_logger();

            // create a TransmissionManager
            let manager = TransmissionManager::new(app.app_handle().clone());

            // store it in Tauri's managed state
            app.manage(AppState {
                tx_manager: manager,
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
