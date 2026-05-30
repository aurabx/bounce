use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Study {
    pub id: Option<i64>,
    pub study_uid: String,
    pub study_description: Option<String>,
    pub institution_name: Option<String>,
    pub institution_address: Option<String>,
    pub patient_id: Option<String>,
    pub other_patient_ids: Option<String>,
    pub accession_no: Option<String>,
    pub patient_name: Option<String>,
    pub issuer_of_patient_id: Option<String>,
    pub patient_birth_date: Option<String>,
    pub patient_sex: Option<String>,
    pub referring_physician_name: Option<String>,
    pub study_date: Option<String>,
    pub study_time: Option<String>,
    pub tz_offset: Option<String>,
    pub images: i64,
    pub series_count: i64,
    pub status: String,
    pub attempts: i64,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

/// Aggregate counts of studies grouped by lifecycle status, plus the most
/// recent reception time. Computed in a single pass over the `studies` table
/// for the dashboard summary cards so the UI never has to load every row to
/// report totals. `pending` is the sum of the not-yet-terminal working states
/// (`IN-PROGRESS`, `QUEUED`, `UPLOADING`, `RETRYING`) and is derived rather
/// than stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total: i64,
    pub in_progress: i64,
    pub queued: i64,
    pub uploading: i64,
    pub retrying: i64,
    pub sent: i64,
    pub failed: i64,
    pub pending: i64,
    pub last_received_at: Option<DateTime<Utc>>,
}

/// A single upload attempt for a study. One row is written when an attempt is
/// claimed (`STARTED`) and updated to `SUCCESS`/`FAILED` when it resolves,
/// forming a per-study audit trail of every upload try.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UploadAttempt {
    pub id: Option<i64>,
    pub study_uid: String,
    pub attempt_no: i64,
    pub upload_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
}
