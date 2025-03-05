use std::sync::Once;
use std::path::Path;
use tracing_subscriber::{self, fmt, prelude::*};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

// Use a static Once guard to ensure initialization happens only once
static INIT: Once = Once::new();

pub fn setup_logger() {
    // This will ensure the code inside only runs once, no matter how many times setup_logger() is called
    INIT.call_once(|| {
        // Create logs directory if it doesn't exist
        let log_dir = "./logs";
        if !Path::new(log_dir).exists() {
            std::fs::create_dir_all(log_dir).expect("Failed to create logs directory");
        }

        // Setup file appender with daily rotation
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            log_dir,
            "application.log",
        );
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        // We need to keep the guard alive for the lifetime of the application
        // Since we can't return it from this function (called multiple times),
        // we intentionally leak it, which is fine for this use case
        Box::leak(Box::new(_guard));

        // Create a multi-layer subscriber that writes to both console and file
        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_ansi(true) // Enable ANSI colors for terminal
                    .with_target(true)
                    .compact()
                    .with_writer(std::io::stdout) // Console output
            )
            .with(
                fmt::layer()
                    .with_ansi(false) // Disable ANSI colors for file
                    .with_target(true)
                    .compact()
                    .with_writer(non_blocking) // File output
            )
            .init();

        tracing::info!("Logger initialized - writing to console and {}/application.log", log_dir);
    });
}

// Optional: Custom logging macro for more flexibility
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*);
    };
}

// Add level-specific log macros
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*);
    };
}