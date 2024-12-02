#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tcp_service;
mod service;

#[tauri::command]
async fn start_server(port: u16) -> Result<(), String> {
  // Pass the port to the TCP server logic
  tcp_service::tcp_background_service::spawn(port)
      .await
      .map_err(|e| format!("Failed to start server: {}", e))?;
  Ok(())
}

#[tauri::command]
async fn start_service(port: u16) -> Result<(), String> {
  // Pass the port to the TCP server logic

  service::service::start()
      .map_err(|e| format!("Failed to start server: {}", e))?;
  Ok(())
}

fn main() {
  tauri::Builder::default()
      .invoke_handler(tauri::generate_handler![start_server])
      .run(tauri::generate_context!())
      .expect("error while running tauri application");
}
