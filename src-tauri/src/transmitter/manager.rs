use crate::log_info;
use crate::transmitter::transmission::Transmission;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct TransmissionManager {
    pub sender: mpsc::Sender<TransmissionCommand>,
}

impl TransmissionManager {
    pub fn new(app_handle: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel(32); // Channel to send/receive commands
        let transmission = Arc::new(Mutex::new(Transmission::new(app_handle)));

        log_info!("TransmissionManager boot...");

        // Start background task
        let _task = tauri::async_runtime::spawn(Self::run_background_task(rx, transmission.clone()));

        log_info!("... TransmissionManager booted");

        Self {
            sender: tx,
        }
    }

    /// Background task to process commands
    async fn run_background_task(
        mut receiver: mpsc::Receiver<TransmissionCommand>,
        transmission: Arc<Mutex<Transmission>>,
    ) {
        log_info!("start run_background_task");

        while let Some(command) = receiver.recv().await {
            log_info!("run_background_task {:?}", command);

            let tx_clone = transmission.clone();
            tokio::spawn(async move {
                let transmission = tx_clone.lock().await;

                match command {
                    // TransmissionCommand::SendStudy { study_uid, delete_after_send } => {
                    //     log_info!("SendStudy: {:?} {:?}", study_uid, delete_after_send);
                    // }
                    TransmissionCommand::ScheduleStudy { study_uid } => {
                        log_info!("ScheduleStudy sending {:?}", study_uid);
                        if let Err(e) = transmission.schedule_study_push(study_uid).await {
                            println!("Error sending study: {:?}", e);
                        }
                    }
                }
            });
        }
    }

    /// Send a command to the transmission background task
    pub async fn send_command(&self, command: TransmissionCommand) {
        log_info!("send_command received");
        let command_clone = command.clone();
        let result = self.sender.send(command).await;

        match result {
            Ok(()) => log_info!("Command sent successfully {:?}", command_clone),
            Err(e) => log_info!("Failed to send command: {:?}", e)
        }
    }
}

/// Commands that the Transmission background task will process
#[allow(dead_code)]
#[derive(Debug)]
pub enum TransmissionCommand {
    ScheduleStudy { study_uid: String },
}

impl TransmissionCommand {
    pub(crate) fn clone(&self) -> TransmissionCommand {
        match self {
            TransmissionCommand::ScheduleStudy { study_uid } => {
                TransmissionCommand::ScheduleStudy {
                    study_uid: study_uid.clone(),
                }
            }
        }
    }
}

impl std::fmt::Display for TransmissionCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransmissionCommand::ScheduleStudy { study_uid } => {
                write!(f, "ScheduleStudy(study_uid: {})", study_uid)
            }
        }
    }
}
