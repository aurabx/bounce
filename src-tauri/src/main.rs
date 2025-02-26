#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod logger;
mod receiver;
mod store;
mod transmitter;
mod lib;

use store::config::Config;
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tauri_plugin_store::StoreExt;
use std::sync::{Arc};
use std::time::Duration;
use crate::lib::task_manager::TaskManager;
use crate::transmitter::manager::{TransmissionCommand, TransmissionManager};
use crate::transmitter::transmission::Transmission;

#[derive(Clone)]
struct AppState {
    // tx_manager: Arc<TransmissionManager>,
    tx_manager: TransmissionManager,
}


#[tauri::command]
fn send_log(app: tauri::AppHandle, log: String) -> Result<(), String> {
    println!("log: {}", log);
    app.emit("log", log).unwrap();

    Ok(())
}

#[tauri::command]
async fn receiver_start(app: AppHandle) -> Result<(), String> {
    println!("receiver_start: {}", "now");

    let store = app
        .store("store.json")
        .map_err(|e| format!("Failed to load store: {}", e))?;

    let config = Config::load(store);

    app.emit("log", "Starting server").unwrap();

    receiver::server::start(config, app)
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let store = app.store("store.json")?;

            // Retrieve API key (with error handling)
            let value = store
                .get("api_key")
                .expect("Failed to get api_key from store");

            println!("api_key: {}", value);

            let config = Config::load(store);
            
            // create a TransmissionManager
            let manager = TransmissionManager::new(config);


            // store it in Tauri's managed state
            app.manage(AppState {
                // tx_manager: Arc::new(manager),
                tx_manager: manager,
            });

            // app.manage(Mutex::new(AppState {
            //     tx_manager: Arc::new(manager),
            // }));

            app.listen("study-received", |event| {
                println!("MAIN: study received {}", event.payload());

                tauri::async_runtime::spawn(async move {
                    //sleep(Duration::from_secs(30)).await;

                    // let state = app.state::<AppState>();
                    //
                    // let study_uid = event.payload().parse().unwrap();
                    //
                    // // state.tx_manager.send_command(TransmissionCommand::ScheduleStudy {
                    // //     study_uid
                    // // }).await;
                    //
                    // transmission.clone().send_study(study_uid, true).await.unwrap();

                    log_info!("MAIN: study-received send_command");
                });
            });
            

            //lib::tray_icon::setup(app);

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            receiver_start,
            receiver_stop,
            send_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
