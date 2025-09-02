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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

