use std::sync::Once;
use tracing_subscriber;

// Use a static Once guard to ensure initialization happens only once
static INIT: Once = Once::new();

pub fn setup_logger() {
    // This will ensure the code inside only runs once, no matter how many times setup_logger() is called
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(true)
            .compact()
            .init();
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