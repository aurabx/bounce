use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use crate::store::config::Config;
use crate::transmitter::transmission::Transmission;

#[derive(Clone)]
pub struct TransmissionManager {
    sender: mpsc::Sender<TransmissionCommand>,
}

impl TransmissionManager {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::channel(32); // Channel to send/receive commands
        let transmission = Arc::new(Mutex::new(Transmission::new(config)));

        // Start background task
        tokio::spawn(Self::run_background_task(rx, transmission));

        Self { sender: tx }
    }

    /// Send a command to the transmission background task
    pub async fn send_command(&self, command: TransmissionCommand) {
        let _ = self.sender.send(command).await;
    }

    /// Background task to handle sending studies
    async fn run_background_task(
        mut receiver: mpsc::Receiver<TransmissionCommand>,
        transmission: Arc<Mutex<Transmission>>,
    ) {
        while let Some(command) = receiver.recv().await {
            let transmission = transmission.clone();
            tokio::spawn(async move {
                let mut transmission = transmission.lock().await;
                match command {
                    TransmissionCommand::SendStudy { study_uid, delete_after_send } => {
                        if let Err(e) = transmission.send_study(study_uid, delete_after_send).await {
                            println!("Error sending study: {:?}", e);
                        }
                    }
                    TransmissionCommand::ScheduleStudy { study_uid } => {
                        if let Err(e) = transmission.schedule_study_push(study_uid).await {
                            println!("Error sending study: {:?}", e);
                        }
                    },
                }
            });
        }
    }
}

/// Commands that the Transmission background task will process
pub enum TransmissionCommand {
    SendStudy { study_uid: String, delete_after_send: bool },
    ScheduleStudy { study_uid: String },
}