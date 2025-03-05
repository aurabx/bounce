#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod logger;
mod receiver;
mod store;
mod transmitter;
mod aura;

use std::sync::Arc;
use store::config::Config;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;
use crate::aura::aura_api::AuraApi;
use crate::receiver::server::init_server_state;
use crate::transmitter::manager::{TransmissionManager};
use crate::transmitter::transmission::Transmission;

#[derive(Clone)]
struct AppState {
    tx_manager: TransmissionManager,
}

fn load_config(app: AppHandle) -> Config {
    let store = app
        .store("store.json")
        .map_err(|e| format!("Failed to load store: {}", e))
        .unwrap();

    Config::load(store)
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
    upload_id: String
) -> Result<(), String> {
    println!(
        "api_start_upload (study_uid: {}), (signature: {}), (signature: {})",
        study_uid, signature, upload_id
    );

    let aura_api = AuraApi::new(load_config(app));

    aura_api.upload_start(
        study_uid,
        signature,
        upload_id
    ).await.expect("upload_start panic");

    Ok(())
}

#[tauri::command]
async fn send_study(
    app: AppHandle,
    study_uid: String,
) -> Result<(), String> {
    println!(
        "send_study (study_uid: {})",
        study_uid
    );

    let transmission = Transmission::new(load_config(app));

    transmission.send_study(
        study_uid,
        false
    ).await.expect("send_study panic");

    Ok(())
}



#[tauri::command]
async fn receiver_start(app: AppHandle) -> Result<(), String> {
    println!("receiver_start: {}", "now");

    app.emit("log", "Starting server").unwrap();

    receiver::server::start(load_config(app.clone()), app)
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

    app.emit("current-studies", config.current_studies()).unwrap();

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // create a TransmissionManager
            let manager = TransmissionManager::new(
                load_config(app.app_handle().clone())
            );

            // store it in Tauri's managed state
            app.manage(AppState {
                tx_manager: manager,
            });

            // Initialize and manage server state
            app.manage(Arc::new(Mutex::new(init_server_state())));

            app.listen("study-received", |event| {
                println!("MAIN: study received {}", event.payload());

                tauri::async_runtime::spawn(async move {
                    log_info!("MAIN: study-received send_command");
                });
            });

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            receiver_start,
            receiver_stop,
            send_log,
            api_start_upload,
            current_studies
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
