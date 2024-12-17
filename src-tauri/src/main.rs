#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod logger;
mod receiver;
mod store;
mod lib;

use store::config::Config;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

#[tauri::command]
fn send_log(app: tauri::AppHandle, log: String) -> Result<(), String> {
    println!("log: {}", log);

    app.emit("log", log).unwrap();

    Ok(())
}

#[tauri::command]
async fn receiver_start(app: tauri::AppHandle, log: String) -> Result<(), String> {
    println!("receiver_start: {}", log);

    let store = app
        .store("store.json")
        .map_err(|e| format!("Failed to load store: {}", e))?;

    let config = Config::load(store);

    app.emit("receiver_start", log).unwrap();

    receiver::server::start(config)
        .await
        .map_err(|e| format!("Failed to start server: {}", e))?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let store = app.store("store.json")?;

            // Retrieve API key (with error handling)
            let value = store
                .get("api_key")
                .expect("Failed to get api_key from store");

            println!("api_key: {}", value);

            //lib::tray_icon::setup(app);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![receiver_start, send_log])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
