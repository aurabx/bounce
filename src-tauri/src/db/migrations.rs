use anyhow::Result;
use sqlx::SqlitePool;

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    // Create studies table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS studies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            study_uid TEXT NOT NULL UNIQUE,
            study_description TEXT,
            institution_name TEXT,
            institution_address TEXT,
            patient_id TEXT,
            other_patient_ids TEXT,
            accession_no TEXT,
            patient_name TEXT,
            issuer_of_patient_id TEXT,
            patient_birth_date TEXT,
            patient_sex TEXT,
            referring_physician_name TEXT,
            study_date TEXT,
            study_time TEXT,
            tz_offset TEXT,
            images INTEGER NOT NULL DEFAULT 0,
            series_count INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'IN-PROGRESS',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            sent_at DATETIME,
            CONSTRAINT unique_study_uid UNIQUE (study_uid)
        );
        "#,
    )
    .execute(pool)
    .await?;

    // Create index on study_uid for faster lookups
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_studies_study_uid ON studies (study_uid);
        "#,
    )
    .execute(pool)
    .await?;

    // Create trigger to update updated_at timestamp on studies
    sqlx::query(
        r#"
        CREATE TRIGGER IF NOT EXISTS update_studies_updated_at
            AFTER UPDATE ON studies
            FOR EACH ROW
        BEGIN
            UPDATE studies SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
        END;
        "#,
    )
    .execute(pool)
    .await?;

    // Add upload retry/recovery columns to the studies table. SQLite has no
    // "ADD COLUMN IF NOT EXISTS", so each ADD is run individually and a
    // "duplicate column name" error is swallowed to keep the migration
    // re-runnable (consistent with the IF NOT EXISTS blocks above).
    add_column_if_missing(pool, "ALTER TABLE studies ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0").await?;
    add_column_if_missing(pool, "ALTER TABLE studies ADD COLUMN last_attempt_at DATETIME").await?;
    add_column_if_missing(pool, "ALTER TABLE studies ADD COLUMN next_retry_at DATETIME").await?;
    add_column_if_missing(pool, "ALTER TABLE studies ADD COLUMN last_error TEXT").await?;

    // Index supporting the retry scheduler's "due for retry" query.
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_studies_status_next_retry
            ON studies (status, next_retry_at);
        "#,
    )
    .execute(pool)
    .await?;

    // Per-attempt upload history. One row per individual upload attempt,
    // giving a full audit trail rather than just the latest error held on
    // the studies row.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS upload_attempts (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            study_uid    TEXT NOT NULL,
            attempt_no   INTEGER NOT NULL,
            upload_id    TEXT,
            status       TEXT NOT NULL,
            error        TEXT,
            started_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at  DATETIME,
            duration_ms  INTEGER,
            CONSTRAINT fk_attempt_study FOREIGN KEY (study_uid) REFERENCES studies (study_uid)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_upload_attempts_study
            ON upload_attempts (study_uid, started_at);
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Run an `ALTER TABLE ... ADD COLUMN` statement, treating an existing column
/// as success so the migration remains idempotent across restarts.
async fn add_column_if_missing(pool: &SqlitePool, sql: &str) -> Result<()> {
    match sqlx::query(sql).execute(pool).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("duplicate column name") {
                Ok(())
            } else {
                Err(e.into())
            }
        }
    }
}
