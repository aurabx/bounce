use crate::db::{migrations, models::*};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use crate::{load_config, log_error, log_info};

pub struct Database {
    pool: SqlitePool,
    app_handle: AppHandle,
}

impl Database {
    pub async fn new(app_handle: AppHandle) -> Result<Self> {
        let app_dir = app_handle.clone()
            .path()
            .app_data_dir()
            .context("Failed to get app data directory")?;



        // Ensure the directory exists
        tokio::fs::create_dir_all(&app_dir).await?;

        let db_path = app_dir.join("bounce.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        log_info!("Connecting to db at: {}", db_url);

        let pool = SqlitePool::connect(&db_url)
            .await
            .context("Failed to connect to SQLite db")?;

        // Run migrations
        migrations::run_migrations(&pool)
            .await
            .context("Failed to run db migrations")?;

        Ok(Self { pool, app_handle })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // Study operations
    pub async fn create_or_update_study(&self, new_study: Study) -> Result<Study> {
        let mut tx = self.pool.begin().await?;

        // Try to insert, if conflict then update
        let result = sqlx::query(
            r#"
            INSERT INTO studies (
                study_uid, study_description, institution_name, institution_address,
                patient_id, other_patient_ids, accession_no, patient_name,
                issuer_of_patient_id, patient_birth_date, patient_sex,
                referring_physician_name, study_date, study_time, tz_offset
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(study_uid) DO UPDATE SET
                study_description = COALESCE(excluded.study_description, study_description),
                institution_name = COALESCE(excluded.institution_name, institution_name),
                institution_address = COALESCE(excluded.institution_address, institution_address),
                patient_id = COALESCE(excluded.patient_id, patient_id),
                other_patient_ids = COALESCE(excluded.other_patient_ids, other_patient_ids),
                accession_no = COALESCE(excluded.accession_no, accession_no),
                patient_name = COALESCE(excluded.patient_name, patient_name),
                issuer_of_patient_id = COALESCE(excluded.issuer_of_patient_id, issuer_of_patient_id),
                patient_birth_date = COALESCE(excluded.patient_birth_date, patient_birth_date),
                patient_sex = COALESCE(excluded.patient_sex, patient_sex),
                referring_physician_name = COALESCE(excluded.referring_physician_name, referring_physician_name),
                study_date = COALESCE(excluded.study_date, study_date),
                study_time = COALESCE(excluded.study_time, study_time),
                tz_offset = COALESCE(excluded.tz_offset, tz_offset)
            "#,
        )
            .bind(&new_study.study_uid)
            .bind(&new_study.study_description)
            .bind(&new_study.institution_name)
            .bind(&new_study.institution_address)
            .bind(&new_study.patient_id)
            .bind(&new_study.other_patient_ids)
            .bind(&new_study.accession_no)
            .bind(&new_study.patient_name)
            .bind(&new_study.issuer_of_patient_id)
            .bind(&new_study.patient_birth_date)
            .bind(&new_study.patient_sex)
            .bind(&new_study.referring_physician_name)
            .bind(&new_study.study_date)
            .bind(&new_study.study_time)
            .bind(&new_study.tz_offset)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        // Fetch and return the study
        self.get_study_by_uid(&new_study.study_uid).await
    }

    pub async fn get_study_by_uid(&self, study_uid: &str) -> Result<Study> {
        let study = sqlx::query_as::<_, Study>(
            "SELECT * FROM studies WHERE study_uid = ?"
        )
            .bind(study_uid)
            .fetch_one(&self.pool)
            .await
            .context("Study not found")?;

        Ok(study)
    }

    pub async fn get_studies(&self) -> Result<Vec<Study>> {
        let studies = sqlx::query_as::<_, Study>(
            "SELECT * FROM studies ORDER BY created_at DESC"
        )
            .fetch_all(&self.pool)
            .await?;

        Ok(studies)
    }

    pub async fn current_studies(&self) -> Value {
        let config = load_config(self.app_handle.clone());

        match self.get_studies().await {
            Ok(studies) => {
                let mut studies_json = Vec::new();

                for study in studies {

                    // Check if the study directory exists
                    let study_path = config.resolve_study_path(&study.study_uid);
                    let exists = study_path.exists();

                    let study_json = serde_json::json!({
                        "study_uid": study.study_uid,
                        "study_description": study.study_description.unwrap_or_else(|| "No study description".to_string()),
                        "study_date": study.study_date.unwrap_or_else(|| "No study date".to_string()),
                        "study_time": study.study_time.unwrap_or_else(|| "No study time".to_string()),
                        "patient_name": study.patient_name.unwrap_or_else(|| "No patient name".to_string()),
                        "patient_id": study.patient_id.unwrap_or_else(|| "No patient ID".to_string()),
                        "institution_name": study.institution_name.unwrap_or_else(|| "No institution".to_string()),
                        "accession_no": study.accession_no.unwrap_or_else(|| "No accession number".to_string()),
                        "patient_sex": study.patient_sex.unwrap_or_else(|| "Unknown".to_string()),
                        "patient_birth_date": study.patient_birth_date.unwrap_or_else(|| "Unknown".to_string()),
                        "referring_physician_name": study.referring_physician_name.unwrap_or_else(|| "Unknown".to_string()),
                        "series_count": study.series_count,
                        "images": study.images,
                        "status": study.status,
                        "created_at": study.created_at,
                        "updated_at": study.updated_at,
                        "sent_at": study.sent_at,
                        "exists": exists,
                        "path": study_path.to_string_lossy(),
                    });

                    studies_json.push(study_json);
                }

                let count = studies_json.len();

                let result = serde_json::json!({
                    "studies": studies_json,
                    "count": count
                });

                result
            } Err(e) => {
                log_error!("Error fetching studies from a database: {}", e);

                let error_result = serde_json::json!({
                    "studies": [],
                    "count": 0,
                    "error": e.to_string()
                });

                error_result
            }
        }
    }


    pub async fn update_study_status(&self, study_uid: &str, status: &str) -> Result<()> {
        let sent_at = if status == "SENT" {
            Some(Utc::now())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE studies
            SET status = ?, sent_at = ?
            WHERE study_uid = ?
            "#,
        )
            .bind(status)
            .bind(sent_at)
            .bind(study_uid)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_study_counts(&self, study_id: i64) -> Result<()> {
        // Update series count
        let series_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM series WHERE study_id = ?"
        )
            .bind(study_id)
            .fetch_one(&self.pool)
            .await?;

        // For image count, we'll need to count files in the filesystem
        // or maintain a separate images table if needed
        sqlx::query(
            "UPDATE studies SET series_count = ? WHERE id = ?"
        )
            .bind(series_count)
            .bind(study_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_study_image_count(&self, study_uid: &str, image_count: i64) -> Result<()> {
        sqlx::query(
            "UPDATE studies SET images = ? WHERE study_uid = ?"
        )
            .bind(image_count)
            .bind(study_uid)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_study(&self, study_uid: String) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Delete series first (due to foreign key constraint)
        sqlx::query("DELETE FROM series WHERE study_instance_uid = ?")
            .bind(study_uid.clone())
            .execute(&mut *tx)
            .await?;

        // Delete study
        sqlx::query("DELETE FROM studies WHERE study_uid = ?")
            .bind(study_uid)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

}