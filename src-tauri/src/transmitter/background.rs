// use tauri::{App, AppHandle, Emitter, Manager};
// use tokio::sync::{mpsc, Mutex};
// use std::sync::Arc;
// use serde::{Deserialize, Serialize};
// use std::collections::HashMap;
// use crate::{log_info, AppState};
//
// // Command types that can be sent to the background task
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum TaskCommand {
//     ScheduleStudy { study_uid: String },
// }
//
// // Background task state
// #[derive(Debug, Clone)]
// pub struct ProcessInfo {
//     pub running: bool,
//     pub progress: u32,
//     pub cancel_tx: Option<mpsc::UnboundedSender<()>>,
// }
// // Background task manager
// pub struct BackgroundTask {
//     command_rx: mpsc::UnboundedReceiver<TaskCommand>,
//     app_handle: AppHandle,
//     processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
// }
//
// impl BackgroundTask {
//     pub fn new(
//         command_rx: mpsc::UnboundedReceiver<TaskCommand>,
//         app_handle: AppHandle,
//         processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
//     ) -> Self {
//         Self {
//             command_rx,
//             app_handle,
//             processes,
//         }
//     }
//
//     // Main background task loop
//     pub async fn run(mut self) {
//         log_info!("Background task started");
//
//         while let Some(command) = self.command_rx.recv().await {
//             log_info!("Background task received command: {:?}", command);
//             match command {
//                 TaskCommand::ScheduleStudy { study_uid } => {
//                     log_info!("ScheduleStudy sending...{:?}", study_uid);
//                 }
//             }
//         }
//
//         log_info!("Background task stopped");
//     }
//
// }
//
// // Setup function to initialize the background task
// pub fn setup_background_task(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
//     let app_handle = app.handle();
//
//     // Create communication channel
//     let (command_tx, command_rx) = mpsc::unbounded_channel();
//
//     // Create shared state
//     let processes = Arc::new(Mutex::new(HashMap::new()));
//
//     // Create background task state
//     let task_state = AppState {
//         command_tx,
//         processes: Arc::clone(&processes),
//     };
//
//     // Store state in Tauri's state management
//     app.manage(task_state);
//
//     // Create and spawn background task
//     let background_task = BackgroundTask::new(command_rx, app_handle.clone(), processes);
//
//     tauri::async_runtime::spawn(async move {
//         background_task.run().await;
//     });
//
//     Ok(())
// }
