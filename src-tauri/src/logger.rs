use std::sync::Once;
use tracing_appender::rolling::{Rotation};
use tracing_subscriber::{self, prelude::*};

// Use a static Once guard to ensure initialization happens only once
static INIT: Once = Once::new();

pub fn setup_logger() {
    // This will ensure the code inside only runs once, no matter how many times setup_logger() is called
    INIT.call_once(|| {
        log::info!("Logger initialized",);
    });
}

// Optional: Custom logging macro for more flexibility
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

// Add level-specific log macros
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
