use dicom::core::chrono;
use log::{Level, LevelFilter, Metadata, Record};
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

// ── Logger architecture ────────────────────────────────────────────────────
//
// `tauri-plugin-log` is the single global logger registered with the `log`
// crate. We attach a custom `TargetKind::Dispatch` target to it (`LogtailFanout`
// below) that forwards records into a process-wide mpsc channel. A background
// task drains that channel and POSTs batches to Better Stack.
//
// Previously this module registered a second global logger via
// `log::set_boxed_logger`, which raced `tauri-plugin-log` and lost, printing
// `attempted to set a logger after the logging system was already initialized`
// at startup and silently breaking remote shipping. The fan-out target keeps
// a single global logger and avoids the race.

// Configuration for Better Stack Logtail.
#[derive(Debug, Clone)]
pub struct LogtailConfig {
    pub enable: bool,
    pub source_token: String,
    pub endpoint: String,
    pub app_name: String,
    pub hostname: String,
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

// ── Static fan-out channel ────────────────────────────────────────────────
// Created once at module init via `init_logtail_channel`. The sender side is
// shared by the fan-out target; the receiver is taken by the background
// sender task started inside `.setup()`.

static LOGTAIL_TX: OnceLock<UnboundedSender<LogMessage>> = OnceLock::new();
static LOGTAIL_RX: Mutex<Option<UnboundedReceiver<LogMessage>>> = Mutex::new(None);
static GLOBAL_ENABLE: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Construct the fan-out mpsc channel. Must be called once, before
/// `tauri::Builder::default()`, so the `LogtailFanout` target has a live
/// sender to push records into during plugin initialization.
pub fn init_logtail_channel() {
    let (tx, rx) = mpsc::unbounded_channel();
    let _ = LOGTAIL_TX.set(tx);
    *LOGTAIL_RX.lock().expect("LOGTAIL_RX mutex poisoned") = Some(rx);
    let _ = GLOBAL_ENABLE.set(Arc::new(AtomicBool::new(false)));
}

/// Build the `fern::Dispatch` chain that fans tauri-plugin-log records into
/// the Logtail channel. Wrap in `TargetKind::Dispatch(...)` when constructing
/// the `tauri_plugin_log::Builder`.
pub fn logtail_dispatch() -> tauri_plugin_log::fern::Dispatch {
    tauri_plugin_log::fern::Dispatch::new().chain(Box::new(LogtailFanout) as Box<dyn log::Log>)
}

/// Spawn the background task that drains the channel and POSTs batches to
/// Better Stack. Call inside `.setup()` once `Config::load` has supplied the
/// API key (used as `app_name`) and the `send_logs` enable flag.
///
/// Idempotent: subsequent calls only refresh the enable flag.
pub fn start_logtail_sender(config: LogtailConfig) {
    if let Some(flag) = GLOBAL_ENABLE.get() {
        flag.store(config.enable, Ordering::Relaxed);
    }

    let receiver = match LOGTAIL_RX
        .lock()
        .expect("LOGTAIL_RX mutex poisoned")
        .take()
    {
        Some(rx) => rx,
        None => return, // sender already started in a previous call
    };

    let enable = GLOBAL_ENABLE
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(AtomicBool::new(config.enable)));

    tauri::async_runtime::spawn(async move {
        run_logtail_sender(config, receiver, enable).await;
    });
}

/// Toggle the remote-logging enable flag at runtime. Backed by the same atomic
/// the sender task reads, so changes take effect on the next record without
/// restarting the logger.
pub fn set_remote_logging_enabled(enabled: bool) {
    if let Some(flag) = GLOBAL_ENABLE.get() {
        flag.store(enabled, Ordering::Relaxed);
    }
}

// ── Fan-out target ────────────────────────────────────────────────────────

struct LogtailFanout;

impl log::Log for LogtailFanout {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Skip serialization entirely when remote logging is off.
        if !GLOBAL_ENABLE
            .get()
            .map(|f| f.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            return;
        }
        let Some(tx) = LOGTAIL_TX.get() else { return };

        let msg = LogMessage {
            level: record.level().to_string().to_lowercase(),
            message: record.args().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            module: record.module_path().map(|s| s.to_string()),
            file: record.file().map(|s| s.to_string()),
            line: record.line(),
        };
        let _ = tx.send(msg);
    }

    fn flush(&self) {}
}

// ── Background shipper ────────────────────────────────────────────────────

async fn run_logtail_sender(
    config: LogtailConfig,
    mut receiver: UnboundedReceiver<LogMessage>,
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
                        // Drop records that arrived in the channel while the
                        // user had remote logging disabled.
                        if !enable.load(Ordering::Relaxed) {
                            continue;
                        }
                        batch.push(log_msg);

                        if (batch.len() >= BATCH_SIZE || last_send.elapsed() >= BATCH_TIMEOUT)
                            && !batch.is_empty()
                        {
                            send_logs_batch(&client, &config, &batch).await;
                            batch.clear();
                            last_send = std::time::Instant::now();
                        }
                    }
                    None => break,
                }
            }

            _ = tokio::time::sleep(BATCH_TIMEOUT) => {
                if !batch.is_empty() && last_send.elapsed() >= BATCH_TIMEOUT {
                    send_logs_batch(&client, &config, &batch).await;
                    batch.clear();
                    last_send = std::time::Instant::now();
                }
            }
        }
    }

    if !batch.is_empty() {
        send_logs_batch(&client, &config, &batch).await;
    }
}

async fn send_logs_batch(client: &Client, config: &LogtailConfig, logs: &[LogMessage]) {
    let payload: Vec<serde_json::Value> = logs
        .iter()
        .map(|entry| {
            json!({
                "dt": entry.timestamp,
                "level": entry.level,
                "message": entry.message,
                "context": {
                    "app": config.app_name,
                    "hostname": config.hostname,
                    "module": entry.module,
                    "file": entry.file,
                    "line": entry.line
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

// `LevelFilter` is unused by this module today but is exported elsewhere; the
// import is retained intentionally to avoid a thrash on dependents.
#[allow(dead_code)]
const _LEVELFILTER_KEEPALIVE: LevelFilter = LevelFilter::Info;

// ── Public logging macros ─────────────────────────────────────────────────
// Route through the standard `log` crate; tauri-plugin-log handles local sinks
// (stdout, file, webview) and the fan-out target ships to Better Stack.

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
