use log::{Level, LevelFilter, Metadata, Record};
use serde_json::json;
use std::sync::Once;
use reqwest::Client;
use tokio::sync::mpsc;
use std::sync::Arc;
use dicom::core::chrono;

// Use a static Once guard to ensure initialization happens only once
static INIT: Once = Once::new();

// Global sender for direct macro access
static mut GLOBAL_SENDER: Option<Arc<mpsc::UnboundedSender<LogMessage>>> = None;

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

        // Store the sender globally for macro access
        unsafe {
            GLOBAL_SENDER = Some(sender.clone());
        }

        // Start background task to send logs
        let config_clone = config.clone();
        tauri::async_runtime::spawn(async move {
            Self::log_sender_task(config_clone, receiver).await;
        });

        Self { config, sender }
    }

    async fn log_sender_task(
        config: LogtailConfig,
        mut receiver: mpsc::UnboundedReceiver<LogMessage>,
    ) {
        let client = Client::new();
        let mut batch = Vec::new();
        let mut last_send = std::time::Instant::now();
        const BATCH_SIZE: usize = 10; // Reduced for testing
        const BATCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2); // Reduced for testing

        println!("Log sender task started, waiting for messages...");

        loop {
            tokio::select! {
                // Receive new log messages
                msg = receiver.recv() => {
                    match msg {
                        Some(log_msg) => {
                            println!("Received log message: {:?}", log_msg);
                            batch.push(log_msg);

                            // Send batch if it's full or timeout reached
                            if batch.len() >= BATCH_SIZE || last_send.elapsed() >= BATCH_TIMEOUT {
                                if !batch.is_empty() {
                                    println!("Sending batch of {} logs", batch.len());
                                    Self::send_logs_batch(&client, &config, &batch).await;
                                    batch.clear();
                                    last_send = std::time::Instant::now();
                                }
                            }
                        }
                        None => {
                            println!("🔚 Log channel closed, shutting down sender task");
                            break;
                        }
                    }
                }

                // Timeout to send remaining logs
                _ = tokio::time::sleep(BATCH_TIMEOUT) => {
                    if !batch.is_empty() && last_send.elapsed() >= BATCH_TIMEOUT {
                        println!("Timeout reached, sending batch of {} logs", batch.len());
                        Self::send_logs_batch(&client, &config, &batch).await;
                        batch.clear();
                        last_send = std::time::Instant::now();
                    }
                }
            }
        }

        // Send any remaining logs before shutting down
        if !batch.is_empty() {
            println!("🔄 Sending final batch of {} logs", batch.len());
            Self::send_logs_batch(&client, &config, &batch).await;
        }
    }

    async fn send_logs_batch(client: &Client, config: &LogtailConfig, logs: &[LogMessage]) {
        if !config.enable {
            return;
        }

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

        println!("🌐 Sending {} logs to {}", payload.len(), config.endpoint);

        let response = client
            .post(&config.endpoint)
            .header("Authorization", format!("Bearer {}", config.source_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!("Successfully sent {} logs to Logtail", payload.len());
                } else {
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
            let log_msg = LogMessage {
                level: record.level().to_string().to_lowercase(),
                message: record.args().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                module: record.module_path().map(|s| s.to_string()),
                file: record.file().map(|s| s.to_string()),
                line: record.line(),
            };

            println!("LogtailLogger::log called: {:?}", log_msg);

            // Send to Logtail (non-blocking)
            if let Err(e) = self.sender.send(log_msg) {
                eprintln!("Failed to send log message: {}", e);
            }
        }
    }

    fn flush(&self) {
        // Implementation for flushing logs if needed
        // In our case, the background task handles batching and sending
    }
}

// Function to send log directly via global sender
pub fn send_log_direct(level: Level, message: String, module: Option<String>, file: Option<String>, line: Option<u32>) {
    unsafe {
        #[allow(static_mut_refs)]
        if let Some(sender) = &GLOBAL_SENDER {
            let log_msg = LogMessage {
                level: level.to_string().to_lowercase(),
                message,
                timestamp: chrono::Utc::now().to_rfc3339(),
                module,
                file,
                line,
            };

            println!("Direct log send: {:?}", log_msg);

            if let Err(e) = sender.send(log_msg) {
                eprintln!("Failed to send direct log message: {}", e);
            }
        } else {
            eprintln!("Global sender not initialized");
        }
    }
}

pub fn setup_logger_with_config(config: LogtailConfig) {
    // This will ensure the code inside only runs once, no matter how many times setup_logger() is called
    INIT.call_once(|| {
        println!("Setting up logger with config: {:?}", config);
        let logger = LogtailLogger::new(config);

        if let Err(e) = log::set_boxed_logger(Box::new(logger)) {
            eprintln!("Failed to initialize logger: {}", e);
        } else {
            log::set_max_level(LevelFilter::Info);
            println!("Logger initialized with Better Stack Logtail integration");

            // Test the logger immediately
            log::info!("Logger initialized with Better Stack Logtail integration");
        }
    });
}

// Updated macros that use both standard logging AND direct sending
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            log::info!("{}", message);
            $crate::logger::send_log_direct(
                log::Level::Info,
                message,
                Some(module_path!().to_string()),
                Some(file!().to_string()),
                Some(line!())
            );
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            log::error!("{}", message);
            $crate::logger::send_log_direct(
                log::Level::Error,
                message,
                Some(module_path!().to_string()),
                Some(file!().to_string()),
                Some(line!())
            );
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            log::warn!("{}", message);
            $crate::logger::send_log_direct(
                log::Level::Warn,
                message,
                Some(module_path!().to_string()),
                Some(file!().to_string()),
                Some(line!())
            );
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            log::debug!("{}", message);
            $crate::logger::send_log_direct(
                log::Level::Debug,
                message,
                Some(module_path!().to_string()),
                Some(file!().to_string()),
                Some(line!())
            );
        }
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            log::trace!("{}", message);
            $crate::logger::send_log_direct(
                log::Level::Trace,
                message,
                Some(module_path!().to_string()),
                Some(file!().to_string()),
                Some(line!())
            );
        }
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
