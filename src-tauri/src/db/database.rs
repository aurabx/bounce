use crate::db::{migrations, models::*};
use crate::{load_config, log_error, log_info};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

/// Study upload lifecycle statuses persisted in `studies.status`.
pub mod study_status {
    /// Receiving images (debounce window). Default on creation.
    pub const IN_PROGRESS: &str = "IN-PROGRESS";
    /// Ready for an upload attempt.
    pub const QUEUED: &str = "QUEUED";
    /// An upload attempt is in flight (claimed).
    pub const UPLOADING: &str = "UPLOADING";
    /// Upload succeeded (terminal).
    pub const SENT: &str = "SENT";
    /// Last attempt failed; awaiting `next_retry_at`.
    pub const RETRYING: &str = "RETRYING";
    /// Exhausted max attempts (terminal until manual retry).
    pub const FAILED: &str = "FAILED";
}

/// Per-attempt statuses persisted in `upload_attempts.status`.
pub mod attempt_status {
    pub const STARTED: &str = "STARTED";
    pub const SUCCESS: &str = "SUCCESS";
    pub const FAILED: &str = "FAILED";
}

#[derive(Clone, Debug)]
pub struct Database {
    pool: SqlitePool,
    app_handle: Option<AppHandle>,
}

impl Database {
    pub async fn new(app_handle: AppHandle) -> Result<Self> {
        let app_dir = app_handle
            .clone()
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

        Ok(Self {
            pool,
            app_handle: Some(app_handle),
        })
    }

    #[allow(dead_code)]
    pub async fn new_for_test(pool: SqlitePool) -> Self {
        migrations::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");
        Self {
            pool,
            app_handle: None,
        }
    }

    // pub fn pool(&self) -> &SqlitePool {
    //     &self.pool
    // }

    // Study operations
    pub async fn create_or_update_study(&self, new_study: Study) -> Result<Study> {
        let mut tx = self.pool.begin().await?;

        // Try to insert, if conflict then update
        let _result = sqlx::query(
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
        let study = sqlx::query_as::<_, Study>("SELECT * FROM studies WHERE study_uid = ?")
            .bind(study_uid)
            .fetch_one(&self.pool)
            .await
            .context("Study not found")?;

        Ok(study)
    }

    pub async fn current_studies(&self, page: u32, limit: u32, search: Option<String>) -> Value {
        let config = if let Some(app_handle) = &self.app_handle {
            load_config(app_handle.clone())
        } else {
            // In test mode, we can't load config from store, so we might need a workaround
            // or just return basic info. For now, let's panic if called in test
            // or return a default config if we can mock it?
            // A better approach is to have the config passed in or return an error.
            // But current_studies returns Value (JSON), so we can return an error JSON.
            log_error!("current_studies called without app_handle");
            return serde_json::json!({"error": "No app_handle available"});
        };

        // Calculate offset
        let offset = (page.saturating_sub(1)) * limit;

        let trimmed_search = search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        match self
            .get_studies_paginated_filtered(
                offset as i64,
                limit as i64,
                trimmed_search.as_deref(),
            )
            .await
        {
            Ok((studies, total_count)) => {
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
                        "attempts": study.attempts,
                        "last_attempt_at": study.last_attempt_at,
                        "next_retry_at": study.next_retry_at,
                        "last_error": study.last_error,
                        "created_at": study.created_at,
                        "updated_at": study.updated_at,
                        "sent_at": study.sent_at,
                        "exists": exists,
                        "path": study_path.to_string_lossy(),
                    });

                    studies_json.push(study_json);
                }

                let total_pages = (total_count + limit as i64 - 1) / limit as i64;
                let has_next_page = page < total_pages as u32;
                let has_previous_page = page > 1;

                let result = serde_json::json!({
                    "studies": studies_json,
                    "pagination": {
                        "current_page": page,
                        "total_pages": total_pages,
                        "total_items": total_count,
                        "limit": limit,
                        "offset": offset,
                        "has_next_page": has_next_page,
                        "has_previous_page": has_previous_page,
                        "items_on_page": studies_json.len(),
                        "search": trimmed_search,
                    }
                });

                result
            }
            Err(e) => {
                log_error!("Error fetching paginated studies from database: {}", e);

                let error_result = serde_json::json!({
                    "studies": [],
                    "pagination": {
                        "current_page": page,
                        "total_pages": 0,
                        "total_items": 0,
                        "limit": limit,
                        "offset": offset,
                        "has_next_page": false,
                        "has_previous_page": false,
                        "items_on_page": 0,
                        "search": trimmed_search,
                    },
                    "error": e.to_string()
                });

                error_result
            }
        }
    }

    /// Aggregate study counts by lifecycle status for the dashboard summary.
    ///
    /// Runs a single conditional-aggregate query rather than loading rows so
    /// the cost is independent of how many studies exist. `COUNT(CASE …)`
    /// yields 0 (not NULL) for absent statuses, so an empty table returns all
    /// zeros without special-casing.
    pub async fn dashboard_stats(&self) -> Result<DashboardStats> {
        #[derive(sqlx::FromRow)]
        struct StatsRow {
            total: i64,
            in_progress: i64,
            queued: i64,
            uploading: i64,
            retrying: i64,
            sent: i64,
            failed: i64,
            last_received_at: Option<DateTime<Utc>>,
        }

        let row = sqlx::query_as::<_, StatsRow>(
            "SELECT \
                COUNT(*) AS total, \
                COUNT(CASE WHEN status = ? THEN 1 END) AS in_progress, \
                COUNT(CASE WHEN status = ? THEN 1 END) AS queued, \
                COUNT(CASE WHEN status = ? THEN 1 END) AS uploading, \
                COUNT(CASE WHEN status = ? THEN 1 END) AS retrying, \
                COUNT(CASE WHEN status = ? THEN 1 END) AS sent, \
                COUNT(CASE WHEN status = ? THEN 1 END) AS failed, \
                MAX(created_at) AS last_received_at \
             FROM studies",
        )
        .bind(study_status::IN_PROGRESS)
        .bind(study_status::QUEUED)
        .bind(study_status::UPLOADING)
        .bind(study_status::RETRYING)
        .bind(study_status::SENT)
        .bind(study_status::FAILED)
        .fetch_one(&self.pool)
        .await
        .context("Failed to compute dashboard stats")?;

        let pending = row.in_progress + row.queued + row.uploading + row.retrying;

        Ok(DashboardStats {
            total: row.total,
            in_progress: row.in_progress,
            queued: row.queued,
            uploading: row.uploading,
            retrying: row.retrying,
            sent: row.sent,
            failed: row.failed,
            pending,
            last_received_at: row.last_received_at,
        })
    }

    #[allow(dead_code)]
    pub async fn get_studies_paginated(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<Study>, i64)> {
        self.get_studies_paginated_filtered(offset, limit, None).await
    }

    /// Paginated study fetch with an optional free-text filter applied
    /// across description, patient name, patient id, accession number,
    /// and Study UID. `search` is treated as a substring match (`LIKE
    /// %term%`). Whitespace-only or `None` is treated as no filter.
    ///
    /// Returns the page rows along with the total count matching the
    /// filter so the caller can compute pagination correctly when the
    /// search is active.
    pub async fn get_studies_paginated_filtered(
        &self,
        offset: i64,
        limit: i64,
        search: Option<&str>,
    ) -> Result<(Vec<Study>, i64)> {
        let trimmed = search.map(str::trim).filter(|s| !s.is_empty());

        match trimmed {
            None => {
                let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM studies")
                    .fetch_one(&self.pool)
                    .await
                    .context("Failed to get total studies count")?;

                let studies = sqlx::query_as::<_, Study>(
                    "SELECT * FROM studies ORDER BY created_at DESC LIMIT ? OFFSET ?",
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch paginated studies")?;

                Ok((studies, total_count))
            }
            Some(term) => {
                let bound = format!("%{}%", term);

                let total_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM studies \
                     WHERE study_description LIKE ?1 \
                        OR patient_name LIKE ?1 \
                        OR patient_id LIKE ?1 \
                        OR accession_no LIKE ?1 \
                        OR study_uid LIKE ?1",
                )
                .bind(&bound)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get filtered studies count")?;

                let studies = sqlx::query_as::<_, Study>(
                    "SELECT * FROM studies \
                     WHERE study_description LIKE ?1 \
                        OR patient_name LIKE ?1 \
                        OR patient_id LIKE ?1 \
                        OR accession_no LIKE ?1 \
                        OR study_uid LIKE ?1 \
                     ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                )
                .bind(&bound)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch filtered paginated studies")?;

                Ok((studies, total_count))
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

    /// Atomically claim a study for an upload attempt.
    ///
    /// Transitions the study to `UPLOADING` only if it is currently in one of
    /// `eligible_statuses`, increments its attempt counter, and records a
    /// `STARTED` row in `upload_attempts`. Returns `(attempt_id, attempt_no)`
    /// when the claim succeeds, or `None` if another worker already holds it
    /// (the conditional update matched no rows). This is the single guard that
    /// prevents the debounce path and the retry scheduler from double-uploading
    /// the same study.
    pub async fn claim_study_for_upload(
        &self,
        study_uid: &str,
        eligible_statuses: &[&str],
        upload_id: &str,
    ) -> Result<Option<(i64, i64)>> {
        let now = Utc::now();
        let placeholders = vec!["?"; eligible_statuses.len()].join(", ");
        let update_sql = format!(
            "UPDATE studies SET status = '{}', last_attempt_at = ?, attempts = attempts + 1 \
             WHERE study_uid = ? AND status IN ({})",
            study_status::UPLOADING,
            placeholders
        );

        let mut tx = self.pool.begin().await?;

        let mut query = sqlx::query(&update_sql).bind(now).bind(study_uid);
        for status in eligible_statuses {
            query = query.bind(*status);
        }
        let result = query.execute(&mut *tx).await?;

        if result.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(None);
        }

        let attempt_no: i64 =
            sqlx::query_scalar("SELECT attempts FROM studies WHERE study_uid = ?")
                .bind(study_uid)
                .fetch_one(&mut *tx)
                .await?;

        let insert = sqlx::query(
            r#"
            INSERT INTO upload_attempts (study_uid, attempt_no, upload_id, status, started_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(study_uid)
        .bind(attempt_no)
        .bind(upload_id)
        .bind(attempt_status::STARTED)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let attempt_id = insert.last_insert_rowid();

        tx.commit().await?;

        Ok(Some((attempt_id, attempt_no)))
    }

    /// Mark a claimed attempt as succeeded. The study's transition to `SENT`
    /// is handled separately via [`Self::update_study_status`].
    pub async fn mark_attempt_success(&self, attempt_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE upload_attempts
            SET status = ?,
                finished_at = ?,
                duration_ms = CAST((julianday('now') - julianday(started_at)) * 86400000 AS INTEGER)
            WHERE id = ?
            "#,
        )
        .bind(attempt_status::SUCCESS)
        .bind(Utc::now())
        .bind(attempt_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark a claimed attempt as failed and move the study into either
    /// `RETRYING` (when `next_retry_at` is `Some`) or terminal `FAILED` (when
    /// `None`, i.e. the caller has exhausted the retry budget). The attempt row
    /// and the study summary columns are updated in one transaction so they
    /// cannot diverge.
    pub async fn mark_upload_failed(
        &self,
        attempt_id: i64,
        study_uid: &str,
        error: &str,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        // Never persist an empty error message on a FAILED row — operators
        // rely on the Transactions view's Error column to tell them what went
        // wrong, and an empty string is indistinguishable from "no record".
        // The caller already formats anyhow chains via `{:#}`, but defaults
        // here keep the contract local to the persistence layer.
        let recorded_error = if error.trim().is_empty() {
            "Upload failed (no error detail recorded)"
        } else {
            error
        };

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE upload_attempts
            SET status = ?,
                error = ?,
                finished_at = ?,
                duration_ms = CAST((julianday('now') - julianday(started_at)) * 86400000 AS INTEGER)
            WHERE id = ?
            "#,
        )
        .bind(attempt_status::FAILED)
        .bind(recorded_error)
        .bind(Utc::now())
        .bind(attempt_id)
        .execute(&mut *tx)
        .await?;

        let new_status = if next_retry_at.is_some() {
            study_status::RETRYING
        } else {
            study_status::FAILED
        };

        sqlx::query(
            r#"
            UPDATE studies
            SET status = ?, last_error = ?, next_retry_at = ?
            WHERE study_uid = ?
            "#,
        )
        .bind(new_status)
        .bind(recorded_error)
        .bind(next_retry_at)
        .bind(study_uid)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Study UIDs eligible for an upload attempt now: `QUEUED` studies and
    /// `RETRYING` studies whose backoff window has elapsed. Ordered oldest
    /// first (NULL `next_retry_at`, i.e. freshly queued, sorts first in SQLite
    /// ascending order).
    pub async fn find_due_for_retry(&self, limit: i64) -> Result<Vec<String>> {
        let now = Utc::now();
        let uids: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT study_uid FROM studies
            WHERE status = ?
               OR (status = ? AND next_retry_at IS NOT NULL AND next_retry_at <= ?)
            ORDER BY next_retry_at ASC
            LIMIT ?
            "#,
        )
        .bind(study_status::QUEUED)
        .bind(study_status::RETRYING)
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(uids)
    }

    /// One-shot recovery run at receiver startup. Re-queues work orphaned by a
    /// crash so the backlog drains without manual intervention:
    /// - `UPLOADING` studies (an attempt was in flight when the process died)
    ///   become `QUEUED`, and their dangling `STARTED` attempt rows are closed
    ///   as `FAILED`.
    /// - Stale `IN-PROGRESS` studies (no update within the debounce grace
    ///   window) become `QUEUED`; recently-updated ones are left alone so a
    ///   study still being received is not grabbed mid-transfer.
    ///
    /// Returns the number of studies re-queued.
    pub async fn recover_pending_on_startup(&self) -> Result<usize> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;

        // Close attempt rows left dangling by a crash.
        sqlx::query(
            r#"
            UPDATE upload_attempts
            SET status = ?, error = 'interrupted by restart', finished_at = ?
            WHERE status = ?
            "#,
        )
        .bind(attempt_status::FAILED)
        .bind(now)
        .bind(attempt_status::STARTED)
        .execute(&mut *tx)
        .await?;

        let orphaned = sqlx::query(
            r#"
            UPDATE studies SET status = ?, next_retry_at = ?
            WHERE status = ?
            "#,
        )
        .bind(study_status::QUEUED)
        .bind(now)
        .bind(study_status::UPLOADING)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // Stale IN-PROGRESS: compare against SQLite's own clock since
        // updated_at is written by the CURRENT_TIMESTAMP trigger.
        let stale = sqlx::query(
            r#"
            UPDATE studies SET status = ?, next_retry_at = ?
            WHERE status = ? AND updated_at < datetime('now', '-5 minutes')
            "#,
        )
        .bind(study_status::QUEUED)
        .bind(now)
        .bind(study_status::IN_PROGRESS)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        Ok((orphaned + stale) as usize)
    }

    /// Reset a study for a manual retry/send (the Retry and Send buttons).
    /// Moves it to `QUEUED` for immediate pickup and clears the last error,
    /// unless an attempt is already in flight (`UPLOADING`), which is left
    /// untouched to avoid spawning a duplicate concurrent upload. The
    /// cumulative `attempts` count and the `upload_attempts` history are
    /// intentionally preserved as an audit trail.
    pub async fn reset_for_manual_retry(&self, study_uid: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE studies
            SET status = ?, next_retry_at = ?, last_error = NULL
            WHERE study_uid = ? AND status != ?
            "#,
        )
        .bind(study_status::QUEUED)
        .bind(Utc::now())
        .bind(study_uid)
        .bind(study_status::UPLOADING)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Stop the automatic retry loop for a `RETRYING` study by moving it to
    /// terminal `FAILED` and clearing its `next_retry_at` so the retry
    /// scheduler ignores it. The conditional `status = RETRYING` predicate
    /// makes this safe against the scheduler racing in to `claim_study_for_upload`
    /// between the user click and this update — if the row was already claimed
    /// (status `UPLOADING`) the update affects zero rows and we report that to
    /// the caller. The historical `attempts` count and `upload_attempts` rows
    /// are preserved so the audit trail still reflects what was tried.
    ///
    /// Returns `true` if the study was transitioned, `false` if it was no
    /// longer in `RETRYING` (already claimed, already terminal, or unknown).
    pub async fn cancel_retry(&self, study_uid: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE studies
            SET status = ?, next_retry_at = NULL, last_error = ?
            WHERE study_uid = ? AND status = ?
            "#,
        )
        .bind(study_status::FAILED)
        .bind("Retries stopped by user")
        .bind(study_uid)
        .bind(study_status::RETRYING)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Full upload-attempt history for a study, newest first.
    pub async fn upload_attempts_for_study(&self, study_uid: &str) -> Result<Vec<UploadAttempt>> {
        let attempts = sqlx::query_as::<_, UploadAttempt>(
            "SELECT * FROM upload_attempts WHERE study_uid = ? ORDER BY started_at DESC",
        )
        .bind(study_uid)
        .fetch_all(&self.pool)
        .await?;

        Ok(attempts)
    }

    /// Paginated upload-attempt fetch across all studies, with an optional
    /// free-text filter applied across Study UID, upload id, status, and the
    /// recorded error. `search` is a substring match (`LIKE %term%`);
    /// whitespace-only or `None` means no filter.
    ///
    /// Rows are returned newest first (`started_at DESC`, then `id DESC` as a
    /// stable tie-breaker for attempts sharing a timestamp). The total count
    /// matching the filter is returned alongside the page so the caller can
    /// compute pagination correctly when the search is active.
    pub async fn get_upload_attempts_paginated_filtered(
        &self,
        offset: i64,
        limit: i64,
        search: Option<&str>,
    ) -> Result<(Vec<UploadAttempt>, i64)> {
        let trimmed = search.map(str::trim).filter(|s| !s.is_empty());

        match trimmed {
            None => {
                let total_count: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM upload_attempts")
                        .fetch_one(&self.pool)
                        .await
                        .context("Failed to get total upload attempts count")?;

                let attempts = sqlx::query_as::<_, UploadAttempt>(
                    "SELECT * FROM upload_attempts \
                     ORDER BY started_at DESC, id DESC LIMIT ? OFFSET ?",
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch paginated upload attempts")?;

                Ok((attempts, total_count))
            }
            Some(term) => {
                let bound = format!("%{}%", term);

                let total_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM upload_attempts \
                     WHERE study_uid LIKE ?1 \
                        OR upload_id LIKE ?1 \
                        OR status LIKE ?1 \
                        OR error LIKE ?1",
                )
                .bind(&bound)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get filtered upload attempts count")?;

                let attempts = sqlx::query_as::<_, UploadAttempt>(
                    "SELECT * FROM upload_attempts \
                     WHERE study_uid LIKE ?1 \
                        OR upload_id LIKE ?1 \
                        OR status LIKE ?1 \
                        OR error LIKE ?1 \
                     ORDER BY started_at DESC, id DESC LIMIT ?2 OFFSET ?3",
                )
                .bind(&bound)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch filtered paginated upload attempts")?;

                Ok((attempts, total_count))
            }
        }
    }

    /// Build the JSON page payload for the Transactions view: the
    /// upload-attempt rows for the requested page plus pagination metadata,
    /// mirroring the shape returned by [`Database::current_studies`]. Unlike
    /// `current_studies`, no study path resolution is needed, so this does not
    /// depend on the app handle or loaded config.
    pub async fn current_upload_attempts(
        &self,
        page: u32,
        limit: u32,
        search: Option<String>,
    ) -> Value {
        let offset = (page.saturating_sub(1)) * limit;

        let trimmed_search = search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        match self
            .get_upload_attempts_paginated_filtered(
                offset as i64,
                limit as i64,
                trimmed_search.as_deref(),
            )
            .await
        {
            Ok((attempts, total_count)) => {
                let total_pages = (total_count + limit as i64 - 1) / limit as i64;
                let has_next_page = page < total_pages as u32;
                let has_previous_page = page > 1;
                // Bind the count before the json! macro: calling `.len()` on a
                // value also interpolated by the macro confuses its type
                // inference (it infers `Option<Vec<_>>`).
                let items_on_page = attempts.len();

                serde_json::json!({
                    "attempts": attempts,
                    "pagination": {
                        "current_page": page,
                        "total_pages": total_pages,
                        "total_items": total_count,
                        "limit": limit,
                        "offset": offset,
                        "has_next_page": has_next_page,
                        "has_previous_page": has_previous_page,
                        "items_on_page": items_on_page,
                        "search": trimmed_search,
                    }
                })
            }
            Err(e) => {
                log_error!("Error fetching paginated upload attempts from database: {}", e);

                serde_json::json!({
                    "attempts": [],
                    "pagination": {
                        "current_page": page,
                        "total_pages": 0,
                        "total_items": 0,
                        "limit": limit,
                        "offset": offset,
                        "has_next_page": false,
                        "has_previous_page": false,
                        "items_on_page": 0,
                        "search": trimmed_search,
                    },
                    "error": e.to_string()
                })
            }
        }
    }

    pub async fn delete_study(&self, study_uid: String) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Delete attempt history first to satisfy the foreign key.
        sqlx::query("DELETE FROM upload_attempts WHERE study_uid = ?")
            .bind(&study_uid)
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

    /// Clear all studies from the database
    /// This will delete all studies and their associated series
    pub async fn clear_studies(&self) -> Result<usize> {
        let mut tx = self.pool.begin().await?;

        // First get the count of studies that will be deleted
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM studies")
            .fetch_one(&mut *tx)
            .await?;

        // Delete attempt history then studies.
        sqlx::query("DELETE FROM upload_attempts")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM studies").execute(&mut *tx).await?;

        tx.commit().await?;

        log_info!("Cleared {} studies from database", count);

        Ok(count as usize)
    }
}
