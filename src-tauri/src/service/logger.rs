use tracing::{info, error, Level};
use tracing_subscriber;
pub fn setup_logger() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();
}



// Optional: Custom logging macro for more flexibility
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        info!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        error!($($arg)*);
    };
}