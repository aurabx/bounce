//! Background retry/recovery scheduler for study uploads.
//!
//! Periodically scans the database for studies that are due for an upload
//! attempt — freshly `QUEUED` work and `RETRYING` studies whose backoff window
//! has elapsed — and dispatches each through [`Transmission::attempt_upload`].
//!
//! Concurrency is bounded by the process-global upload semaphore inside
//! `attempt_upload`; this loop additionally tracks in-flight study UIDs so it
//! does not repeatedly spawn redundant tasks for a study that is already parked
//! waiting on a permit. Structured to mirror [`crate::query::poller`].

use crate::log_info;
use crate::transmitter::transmission::Transmission;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{oneshot, Mutex};

/// Interval between scans for studies due to be retried.
const RETRY_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Upper bound on how many due studies are pulled per scan. Generous: the
/// global upload semaphore is the real throughput limit, and the in-flight set
/// prevents redundant dispatch.
const RETRY_BATCH_LIMIT: i64 = 100;

/// State for the retry scheduler, holding the shutdown channel sender.
pub struct RetrySchedulerState {
    pub shutdown_sender: Option<oneshot::Sender<()>>,
}

pub fn init_retry_scheduler_state() -> RetrySchedulerState {
    RetrySchedulerState {
        shutdown_sender: None,
    }
}

/// Start the retry scheduler as a background task. Runs until the shutdown
/// signal is sent through [`RetrySchedulerState`] or the task is dropped.
pub fn start_retry_scheduler(app: AppHandle, shutdown_rx: oneshot::Receiver<()>) {
    tokio::spawn(async move {
        log_info!("Retry scheduler started");
        run_retry_loop(app, shutdown_rx).await;
        log_info!("Retry scheduler stopped");
    });
}

async fn run_retry_loop(app: AppHandle, mut shutdown_rx: oneshot::Receiver<()>) {
    let transmission = Transmission::new(app.clone());
    let in_flight: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                log_info!("Retry scheduler: shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(RETRY_POLL_INTERVAL) => {
                dispatch_due(&transmission, &in_flight).await;
            }
        }
    }
}

/// One scan cycle: fetch due study UIDs and spawn a bounded upload attempt for
/// each one that is not already in flight.
async fn dispatch_due(transmission: &Transmission, in_flight: &Arc<Mutex<HashSet<String>>>) {
    let due = match transmission.database().find_due_for_retry(RETRY_BATCH_LIMIT).await {
        Ok(uids) => uids,
        Err(e) => {
            crate::log_error!("Retry scheduler: failed to query due studies: {}", e);
            return;
        }
    };

    if due.is_empty() {
        return;
    }

    log_info!("Retry scheduler: {} studies due for upload", due.len());

    for study_uid in due {
        // Skip studies already dispatched this cycle (or a prior cycle) and
        // still parked on the upload semaphore.
        {
            let mut guard = in_flight.lock().await;
            if !guard.insert(study_uid.clone()) {
                continue;
            }
        }

        let transmission = transmission.clone();
        let in_flight = Arc::clone(in_flight);
        tokio::spawn(async move {
            transmission.attempt_upload(study_uid.clone()).await;
            in_flight.lock().await.remove(&study_uid);
        });
    }
}
