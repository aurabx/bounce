#[cfg(test)]
mod tests {
    use super::super::database::Database;
    use super::super::models::Study;
    use chrono::Utc;
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    // Helper to create a test database in a temporary directory
    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePool::connect(&db_url)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        crate::db::migrations::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        (pool, temp_dir)
    }

    fn create_test_study(study_uid: &str) -> Study {
        Study {
            id: None,
            study_uid: study_uid.to_string(),
            study_description: Some("Test Study".to_string()),
            institution_name: Some("Test Hospital".to_string()),
            institution_address: Some("123 Test St".to_string()),
            patient_id: Some("P12345".to_string()),
            other_patient_ids: None,
            accession_no: Some("ACC123".to_string()),
            patient_name: Some("Test Patient".to_string()),
            issuer_of_patient_id: Some("TEST_ISSUER".to_string()),
            patient_birth_date: Some("19900101".to_string()),
            patient_sex: Some("M".to_string()),
            referring_physician_name: Some("Dr. Smith".to_string()),
            study_date: Some("20240101".to_string()),
            study_time: Some("120000".to_string()),
            tz_offset: Some("+0000".to_string()),
            images: 10,
            series_count: 2,
            status: "PENDING".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            sent_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_study() {
        let (pool, _temp_dir) = setup_test_db().await;
        let study = create_test_study("1.2.3.4.5");

        let result = sqlx::query(
            r#"
            INSERT INTO studies (
                study_uid, study_description, institution_name, institution_address,
                patient_id, accession_no, patient_name,
                patient_birth_date, patient_sex, referring_physician_name,
                study_date, study_time, images, series_count, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&study.study_uid)
        .bind(&study.study_description)
        .bind(&study.institution_name)
        .bind(&study.institution_address)
        .bind(&study.patient_id)
        .bind(&study.accession_no)
        .bind(&study.patient_name)
        .bind(&study.patient_birth_date)
        .bind(&study.patient_sex)
        .bind(&study.referring_physician_name)
        .bind(&study.study_date)
        .bind(&study.study_time)
        .bind(study.images)
        .bind(study.series_count)
        .bind(&study.status)
        .execute(&pool)
        .await;

        assert!(result.is_ok(), "Failed to create study");

        // Verify the study was created
        let retrieved: Study =
            sqlx::query_as::<_, Study>("SELECT * FROM studies WHERE study_uid = ?")
                .bind(&study.study_uid)
                .fetch_one(&pool)
                .await
                .expect("Failed to retrieve study");

        assert_eq!(retrieved.study_uid, study.study_uid);
        assert_eq!(retrieved.patient_name, study.patient_name);
    }

    #[tokio::test]
    async fn test_update_study_on_conflict() {
        let (pool, _temp_dir) = setup_test_db().await;
        let study_uid = "1.2.3.4.5";
        let mut study = create_test_study(study_uid);

        // Insert initial study
        sqlx::query(
            r#"
            INSERT INTO studies (
                study_uid, study_description, patient_name, images, series_count, status
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&study.study_uid)
        .bind(&study.study_description)
        .bind(&study.patient_name)
        .bind(study.images)
        .bind(study.series_count)
        .bind(&study.status)
        .execute(&pool)
        .await
        .expect("Failed to insert study");

        // Update with conflict resolution
        study.study_description = Some("Updated Study Description".to_string());
        study.images = 20;

        sqlx::query(
            r#"
            INSERT INTO studies (study_uid, study_description, images, series_count, status)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(study_uid) DO UPDATE SET
                study_description = COALESCE(excluded.study_description, study_description),
                images = excluded.images
            "#,
        )
        .bind(&study.study_uid)
        .bind(&study.study_description)
        .bind(study.images)
        .bind(study.series_count)
        .bind(&study.status)
        .execute(&pool)
        .await
        .expect("Failed to update study");

        // Verify update
        let retrieved: Study =
            sqlx::query_as::<_, Study>("SELECT * FROM studies WHERE study_uid = ?")
                .bind(study_uid)
                .fetch_one(&pool)
                .await
                .expect("Failed to retrieve updated study");

        assert_eq!(
            retrieved.study_description.unwrap(),
            "Updated Study Description"
        );
        assert_eq!(retrieved.images, 20);
    }

    #[tokio::test]
    async fn test_update_study_status() {
        let (pool, _temp_dir) = setup_test_db().await;
        let study = create_test_study("1.2.3.4.6");

        // Insert study
        sqlx::query(
            r#"
            INSERT INTO studies (study_uid, status, images, series_count)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&study.study_uid)
        .bind(&study.status)
        .bind(study.images)
        .bind(study.series_count)
        .execute(&pool)
        .await
        .expect("Failed to insert study");

        // Update status to SENT
        let new_status = "SENT";
        let sent_at = Some(Utc::now());

        sqlx::query(
            r#"
            UPDATE studies
            SET status = ?, sent_at = ?
            WHERE study_uid = ?
            "#,
        )
        .bind(new_status)
        .bind(sent_at)
        .bind(&study.study_uid)
        .execute(&pool)
        .await
        .expect("Failed to update status");

        // Verify status update
        let retrieved: Study =
            sqlx::query_as::<_, Study>("SELECT * FROM studies WHERE study_uid = ?")
                .bind(&study.study_uid)
                .fetch_one(&pool)
                .await
                .expect("Failed to retrieve study");

        assert_eq!(retrieved.status, new_status);
        assert!(retrieved.sent_at.is_some());
    }

    #[tokio::test]
    async fn test_update_study_image_count() {
        let (pool, _temp_dir) = setup_test_db().await;
        let study = create_test_study("1.2.3.4.7");

        // Insert study
        sqlx::query(
            r#"
            INSERT INTO studies (study_uid, images, series_count, status)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&study.study_uid)
        .bind(study.images)
        .bind(study.series_count)
        .bind(&study.status)
        .execute(&pool)
        .await
        .expect("Failed to insert study");

        // Update image count
        let new_count = 25;
        sqlx::query("UPDATE studies SET images = ? WHERE study_uid = ?")
            .bind(new_count)
            .bind(&study.study_uid)
            .execute(&pool)
            .await
            .expect("Failed to update image count");

        // Verify update
        let retrieved: Study =
            sqlx::query_as::<_, Study>("SELECT * FROM studies WHERE study_uid = ?")
                .bind(&study.study_uid)
                .fetch_one(&pool)
                .await
                .expect("Failed to retrieve study");

        assert_eq!(retrieved.images, new_count);
    }

    #[tokio::test]
    async fn test_delete_study() {
        let (pool, _temp_dir) = setup_test_db().await;
        let study = create_test_study("1.2.3.4.8");

        // Insert study
        sqlx::query(
            r#"
            INSERT INTO studies (study_uid, status, images, series_count)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&study.study_uid)
        .bind(&study.status)
        .bind(study.images)
        .bind(study.series_count)
        .execute(&pool)
        .await
        .expect("Failed to insert study");

        // Delete study
        sqlx::query("DELETE FROM studies WHERE study_uid = ?")
            .bind(&study.study_uid)
            .execute(&pool)
            .await
            .expect("Failed to delete study");

        // Verify deletion
        let result = sqlx::query_as::<_, Study>("SELECT * FROM studies WHERE study_uid = ?")
            .bind(&study.study_uid)
            .fetch_optional(&pool)
            .await
            .expect("Query failed");

        assert!(result.is_none(), "Study should be deleted");
    }

    #[tokio::test]
    async fn test_clear_studies() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Insert multiple studies
        for i in 1..=5 {
            let study = create_test_study(&format!("1.2.3.4.{}", i));
            sqlx::query(
                r#"
                INSERT INTO studies (study_uid, status, images, series_count)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(&study.study_uid)
            .bind(&study.status)
            .bind(study.images)
            .bind(study.series_count)
            .execute(&pool)
            .await
            .expect("Failed to insert study");
        }

        // Verify count before clearing
        let count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM studies")
            .fetch_one(&pool)
            .await
            .expect("Failed to count studies");
        assert_eq!(count_before, 5);

        // Clear all studies
        sqlx::query("DELETE FROM studies")
            .execute(&pool)
            .await
            .expect("Failed to clear studies");

        // Verify count after clearing
        let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM studies")
            .fetch_one(&pool)
            .await
            .expect("Failed to count studies");
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn test_get_studies_paginated() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Insert 15 studies
        for i in 1..=15 {
            let study = create_test_study(&format!("1.2.3.4.{}", i));
            sqlx::query(
                r#"
                INSERT INTO studies (study_uid, status, images, series_count)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(&study.study_uid)
            .bind(&study.status)
            .bind(study.images)
            .bind(study.series_count)
            .execute(&pool)
            .await
            .expect("Failed to insert study");
        }

        // Test pagination - page 1, limit 10
        let page1: Vec<Study> = sqlx::query_as::<_, Study>(
            "SELECT * FROM studies ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(10i64)
        .bind(0i64)
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch page 1");

        assert_eq!(page1.len(), 10);

        // Test pagination - page 2, limit 10
        let page2: Vec<Study> = sqlx::query_as::<_, Study>(
            "SELECT * FROM studies ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(10i64)
        .bind(10i64)
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch page 2");

        assert_eq!(page2.len(), 5);

        // Verify total count
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM studies")
            .fetch_one(&pool)
            .await
            .expect("Failed to count studies");
        assert_eq!(total, 15);
    }

    #[tokio::test]
    async fn test_study_unique_constraint() {
        let (pool, _temp_dir) = setup_test_db().await;
        let study_uid = "1.2.3.4.9";

        // Insert first study
        let result1 = sqlx::query(
            r#"
            INSERT INTO studies (study_uid, status, images, series_count)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(study_uid)
        .bind("PENDING")
        .bind(10)
        .bind(2)
        .execute(&pool)
        .await;

        assert!(result1.is_ok());

        // Attempt to insert duplicate study_uid (should fail)
        let result2 = sqlx::query(
            r#"
            INSERT INTO studies (study_uid, status, images, series_count)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(study_uid)
        .bind("PENDING")
        .bind(10)
        .bind(2)
        .execute(&pool)
        .await;

        assert!(result2.is_err(), "Duplicate study_uid should fail");
    }

    #[tokio::test]
    async fn test_database_struct_methods() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db = Database::new_for_test(pool).await;

        // Create a study using the struct method
        let study = create_test_study("1.2.3");
        let created = db.create_or_update_study(study.clone()).await.unwrap();
        assert_eq!(created.study_uid, "1.2.3");

        // Retrieve it
        let retrieved = db.get_study_by_uid("1.2.3").await.unwrap();
        assert_eq!(retrieved.study_uid, "1.2.3");

        // Test pagination method
        let (studies, count) = db.get_studies_paginated(0, 10).await.unwrap();
        assert_eq!(studies.len(), 1);
        assert_eq!(count, 1);

        // Test update status
        db.update_study_status("1.2.3", "SENT").await.unwrap();
        let retrieved_after_status = db.get_study_by_uid("1.2.3").await.unwrap();
        assert_eq!(retrieved_after_status.status, "SENT");
        assert!(retrieved_after_status.sent_at.is_some());

        // Test delete
        db.delete_study("1.2.3".to_string()).await.unwrap();
        let result = db.get_study_by_uid("1.2.3").await;
        assert!(result.is_err());
    }
}
