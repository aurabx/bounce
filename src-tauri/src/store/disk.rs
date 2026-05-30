//! Disk-space guard for the study storage directory.
//!
//! Studies are written to disk on arrival and accumulate while uploads are
//! pending, so a stalled upload endpoint can fill the volume. These helpers
//! check free space against user-configured thresholds, surface a warning to
//! the UI, and let the upload path pause new work before the disk is exhausted.

use crate::store::config::Config;
use crate::{log_error, log_info};
use tauri::{AppHandle, Emitter};

const BYTES_PER_MB: u64 = 1024 * 1024;

/// A free-space reading for the storage volume, compared against the warn and
/// critical thresholds from configuration.
#[derive(Debug, Clone, Copy)]
pub struct DiskStatus {
    pub available_bytes: u64,
    pub warn_bytes: u64,
    pub critical_bytes: u64,
}

impl DiskStatus {
    pub fn is_warn(&self) -> bool {
        self.available_bytes < self.warn_bytes
    }

    pub fn is_critical(&self) -> bool {
        self.available_bytes < self.critical_bytes
    }
}

/// Read free space on the volume backing the study storage directory.
///
/// Returns `None` when the directory does not yet exist or the platform query
/// fails; callers treat that as "unknown, do not block" so a transient error
/// never stops clinical data being received.
pub fn check_disk(config: &Config) -> Option<DiskStatus> {
    let base_dir = config.get_base_dir();
    match fs2::available_space(&base_dir) {
        Ok(available_bytes) => Some(DiskStatus {
            available_bytes,
            warn_bytes: config.disk_warn_mb.saturating_mul(BYTES_PER_MB),
            critical_bytes: config.disk_critical_mb.saturating_mul(BYTES_PER_MB),
        }),
        Err(e) => {
            log_error!("Disk space check failed for {}: {}", base_dir, e);
            None
        }
    }
}

/// Check free space and, if below the warning threshold, emit a `disk-warning`
/// event for the UI banner and log it. Returns `true` when space is
/// *critically* low, signalling that the caller should pause starting new
/// uploads (which would otherwise create large temporary archives).
pub fn warn_if_low(app: &AppHandle, config: &Config) -> bool {
    let Some(status) = check_disk(config) else {
        return false;
    };

    if status.is_warn() {
        let _ = app.emit(
            "disk-warning",
            serde_json::json!({
                "available_bytes": status.available_bytes,
                "warn_bytes": status.warn_bytes,
                "critical_bytes": status.critical_bytes,
                "critical": status.is_critical(),
            }),
        );
        log_info!(
            "Low disk space: {} MB free (warn < {} MB, critical < {} MB)",
            status.available_bytes / BYTES_PER_MB,
            config.disk_warn_mb,
            config.disk_critical_mb,
        );
    }

    status.is_critical()
}
