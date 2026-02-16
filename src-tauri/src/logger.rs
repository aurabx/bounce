use dicom::core::chrono;
use log::{Level, LevelFilter, Metadata, Record};
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Once;
use tokio::sync::mpsc;

// Use a static Once guard to ensure initialization happens only once
static INIT: Once = Once::new();

// Global enable flag — can be toggled at runtime without restarting the logger
static mut GLOBAL_ENABLE: Option<Arc<AtomicBool>> = None;

// Configuration for Better Stack Logtail
#[derive(Debug, Clone)]
pub struct LogtailConfig {
    pub enable: bool,
    pub source_token: String,
    pub endpoint: String,
    pub app_name: String,
    pub hostname: String,
}

// Custom logger that sends to Better Stack Logtail
pub struct LogtailLogger {
    #[allow(dead_code)]
    config: LogtailConfig,
    sender: Arc<mpsc::UnboundedSender<LogMessage>>,
    enable: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
struct LogMessage {
    level: String,
    message: String,
    timestamp: String,
    module: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

impl LogtailLogger {
    pub fn new(config: LogtailConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let sender = Arc::new(sender);
        let enable = Arc::new(AtomicBool::new(config.enable));

        // Store the enable flag globally so it can be updated later
        unsafe {
            GLOBAL_ENABLE = Some(enable.clone());
        }

        // Start background task to send logs
        let config_clone = config.clone();
        let enable_clone = enable.clone();
        tauri::async_runtime::spawn(async move {
            Self::log_sender_task(config_clone, receiver, enable_clone).await;
        });

        Self {
            config,
            sender,
            enable,
        }
    }

    async fn log_sender_task(
        config: LogtailConfig,
        mut receiver: mpsc::UnboundedReceiver<LogMessage>,
        enable: Arc<AtomicBool>,
    ) {
        let client = Client::new();
        let mut batch = Vec::new();
        let mut last_send = std::time::Instant::now();
        const BATCH_SIZE: usize = 10;
        const BATCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

        loop {
            tokio::select! {
                msg = receiver.recv() => {
                    match msg {
                        Some(log_msg) => {
                            // Skip queueing if remote logging is disabled
                            if !enable.load(Ordering::Relaxed) {
                                continue;
                            }

                            batch.push(log_msg);

                            if (batch.len() >= BATCH_SIZE || last_send.elapsed() >= BATCH_TIMEOUT) && !batch.is_empty() {
                                Self::send_logs_batch(&client, &config, &batch).await;
                                batch.clear();
                                last_send = std::time::Instant::now();
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }

                _ = tokio::time::sleep(BATCH_TIMEOUT) => {
                    if !batch.is_empty() && last_send.elapsed() >= BATCH_TIMEOUT {
                        Self::send_logs_batch(&client, &config, &batch).await;
                        batch.clear();
                        last_send = std::time::Instant::now();
                    }
                }
            }
        }

        // Send any remaining logs before shutting down
        if !batch.is_empty() {
            Self::send_logs_batch(&client, &config, &batch).await;
        }
    }

    async fn send_logs_batch(client: &Client, config: &LogtailConfig, logs: &[LogMessage]) {
        let payload: Vec<serde_json::Value> = logs
            .iter()
            .map(|log| {
                json!({
                    "dt": log.timestamp,
                    "level": log.level,
                    "message": log.message,
                    "context": {
                        "app": config.app_name,
                        "hostname": config.hostname,
                        "module": log.module,
                        "file": log.file,
                        "line": log.line
                    }
                })
            })
            .collect();

        let response = client
            .post(&config.endpoint)
            .header("Authorization", format!("Bearer {}", config.source_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    eprintln!("Failed to send logs to Logtail: HTTP {} - {}", status, body);
                }
            }
            Err(e) => {
                eprintln!("Error sending logs to Logtail: {}", e);
            }
        }
    }
}

impl log::Log for LogtailLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // Print to stdout for local development
            println!("[{}] {}", record.level(), record.args());

            // Only queue for remote sending if enabled
            if !self.enable.load(Ordering::Relaxed) {
                return;
            }

            let log_msg = LogMessage {
                level: record.level().to_string().to_lowercase(),
                message: record.args().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                module: record.module_path().map(|s| s.to_string()),
                file: record.file().map(|s| s.to_string()),
                line: record.line(),
            };

            if let Err(e) = self.sender.send(log_msg) {
                eprintln!("Failed to send log message: {}", e);
            }
        }
    }

    fn flush(&self) {
        // The background task handles batching and sending
    }
}

pub fn setup_logger_with_config(config: LogtailConfig) {
    INIT.call_once(|| {
        let logger = LogtailLogger::new(config);

        if let Err(e) = log::set_boxed_logger(Box::new(logger)) {
            eprintln!("Failed to initialize logger: {}", e);
        } else {
            log::set_max_level(LevelFilter::Info);
            log::info!("Logger initialized");
        }
    });
}

/// Update the remote logging enable flag at runtime (e.g. when the user
/// toggles "Send logs to Aurabox team" in Settings).
pub fn set_remote_logging_enabled(enabled: bool) {
    unsafe {
        #[allow(static_mut_refs)]
        if let Some(flag) = &GLOBAL_ENABLE {
            flag.store(enabled, Ordering::Relaxed);
        }
    }
}

// Macros that route through the standard `log` crate.
// LogtailLogger::log() handles both local printing and remote queueing.

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        log::info!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        log::error!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        log::warn!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        log::debug!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        log::trace!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_info_with_context {
    ($message:expr, $context:expr) => {
        $crate::logger::log_with_context(log::Level::Info, $message, $context);
    };
}

#[macro_export]
macro_rules! log_error_with_context {
    ($message:expr, $context:expr) => {
        $crate::logger::log_with_context(log::Level::Error, $message, $context);
    };
}
