use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use crate::log_info;
use crate::store::config::Config;
use crate::transmitter::transmission::Transmission;

#[derive(Debug, Clone)]
pub struct TransmissionManager {
    sender: mpsc::Sender<TransmissionCommand>,
}

impl TransmissionManager {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::channel(32); // Channel to send/receive commands
        let transmission = Arc::new(Mutex::new(Transmission::new(config)));

        // Start background task
        let _ =  tauri::async_runtime::spawn({
            Self::run_background_task(rx, transmission)
        });


        Self {
            sender: tx
        }
    }

    /// Background task to process commands
    async fn run_background_task(
        mut receiver: mpsc::Receiver<TransmissionCommand>,
        transmission: Arc<Mutex<Transmission>>,
    ) {
        while let Some(command) = receiver.recv().await {
            let transmission = transmission.clone();
            tokio::spawn(async move {
                let transmission = transmission.lock().await;

                match command {
                    TransmissionCommand::SendStudy { study_uid, delete_after_send } => {
                        log_info!("SendStudy: {:?} {:?}", study_uid, delete_after_send);
                    }
                    TransmissionCommand::ScheduleStudy { study_uid } => {
                        log_info!("ScheduleStudy sending...");
                        if let Err(e) = transmission.schedule_study_push(study_uid).await {
                            println!("Error sending study: {:?}", e);
                        }
                    },
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
pub enum TransmissionCommand {
    SendStudy { study_uid: String, delete_after_send: bool },
    ScheduleStudy { study_uid: String },
}