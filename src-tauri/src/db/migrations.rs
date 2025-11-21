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

    Ok(())
}
