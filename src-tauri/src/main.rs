#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use tauri::{Emitter, Manager};
use tauri_plugin_store::{Store, StoreBuilder};
use tauri_plugin_store::StoreExt;
use serde_json::json;
use std::collections::HashMap;

mod logger;
mod service;

#[tauri::command]
async fn start_service(app_handle: tauri::AppHandle, port: u16) -> Result<(), String> {

    let store = app_handle.store("store.json")
        .map_err(|e| format!("Failed to load store: {}", e))?;

    let api_key = store.get("api_key")
        .ok_or_else(|| "API key not found in store".to_string())?
        .as_str()
        .ok_or_else(|| "API key is not a string".to_string())?
        .to_string();


    println!("api_key in command: {}", api_key);
    let config = store.entries();
    let mut keyed_config = HashMap::new();
    for (key, value) in config {
        keyed_config.insert(key, value);

    }
    println!("store.entries(): {}", serde_json::to_string(&keyed_config).unwrap().to_string());

    service::server::start(port)
        .await
        .map_err(|e| format!("Failed to start server: {}", e))?;
    Ok(())
}

#[tauri::command]
fn send_log(app: tauri::AppHandle, log: String) -> Result<(), String> {

    println!("log: {}", log);

    app.emit("log", log).unwrap();

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![start_service])
        .invoke_handler(tauri::generate_handler![send_log])
        .setup(|app| {
            // This loads the store from disk
            let store = app.store("store.json")?;
            let struct_app_handle = app.handle().clone();

            // Note that values must be serde_json::Value instances,
            // otherwise, they will not be compatible with the JavaScript bindings.
            // store.set("a".to_string(), json!("b"));

            let value = store.get("api_key").expect("Failed to get api_key from store");
            println!("api_key: {}", value);


            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
