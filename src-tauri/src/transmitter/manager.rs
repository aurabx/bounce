use crate::log_info;
use crate::transmitter::transmission::Transmission;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::async_runtime::JoinHandle;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct TransmissionManager {
    sender: mpsc::Sender<TransmissionCommand>,
    background_task: Arc<Mutex<Option<JoinHandle<()>>>>, // Store the task handle
}

impl TransmissionManager {
    pub fn new(app_handle: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel(32); // Channel to send/receive commands
        let transmission = Arc::new(Mutex::new(Transmission::new(app_handle)));

        // Start background task
        //let background_task = tauri::async_runtime::spawn(Self::run_background_task(rx, transmission));

        let background_task = Arc::new(Mutex::new(Some(
            tauri::async_runtime::spawn(Self::run_background_task(rx, transmission.clone()))
        )));

        Self { sender: tx, background_task }
    }

    /// Background task to process commands
    async fn run_background_task(
        mut receiver: mpsc::Receiver<TransmissionCommand>,
        transmission: Arc<Mutex<Transmission>>,
    ) {
        while let Some(command) = receiver.recv().await {
            log_info!("run_background_task receiver...");
            let transmission = transmission.clone();
            tokio::spawn(async move {
                let transmission = transmission.lock().await;

                match command {
                    // TransmissionCommand::SendStudy { study_uid, delete_after_send } => {
                    //     log_info!("SendStudy: {:?} {:?}", study_uid, delete_after_send);
                    // }
                    TransmissionCommand::ScheduleStudy { study_uid } => {
                        log_info!("ScheduleStudy sending...{:?}", study_uid);
                        // if let Err(e) = transmission.schedule_study_push(study_uid).await {
                        //     println!("Error sending study: {:?}", e);
                        // }
                    }
                }
            });
        }
    }

    /// Send a command to the transmission background task
    pub async fn send_command(&self, command: TransmissionCommand) {
        log_info!("send_command received");

        let _ = self.sender.send(command).await;
    }

}

/// Commands that the Transmission background task will process
#[allow(dead_code)]
pub enum TransmissionCommand {
    // SendStudy { study_uid: String, delete_after_send: bool },
    ScheduleStudy { study_uid: String },
}
