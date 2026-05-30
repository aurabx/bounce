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
            attempts: 0,
            last_attempt_at: None,
            next_retry_at: None,
            last_error: None,
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

    #[tokio::test]
    async fn test_get_studies_paginated_filtered() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db = Database::new_for_test(pool).await;

        let mut ankle = create_test_study("1.2.3.4.ankle");
        ankle.study_description = Some("US Ankle Right".to_string());
        ankle.patient_name = Some("SMITH^JOHN".to_string());
        ankle.patient_id = Some("P-ANK-001".to_string());
        ankle.accession_no = Some("ACC-ANK-1".to_string());
        db.create_or_update_study(ankle).await.unwrap();

        let mut chest = create_test_study("1.2.3.4.chest");
        chest.study_description = Some("CT Chest".to_string());
        chest.patient_name = Some("DOE^JANE".to_string());
        chest.patient_id = Some("P-CH-002".to_string());
        chest.accession_no = Some("ACC-CH-2".to_string());
        db.create_or_update_study(chest).await.unwrap();

        let mut spine = create_test_study("1.2.3.4.spine");
        spine.study_description = Some("MRI Spine".to_string());
        spine.patient_name = Some("ROE^RICHARD".to_string());
        spine.patient_id = Some("P-SP-003".to_string());
        spine.accession_no = Some("ACC-SP-3".to_string());
        db.create_or_update_study(spine).await.unwrap();

        // Match by description token
        let (studies, count) = db
            .get_studies_paginated_filtered(0, 10, Some("ankle"))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(studies.len(), 1);
        assert_eq!(studies[0].study_uid, "1.2.3.4.ankle");

        // Match by patient surname
        let (studies, count) = db
            .get_studies_paginated_filtered(0, 10, Some("DOE"))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(studies[0].study_uid, "1.2.3.4.chest");

        // Match by accession substring
        let (studies, count) = db
            .get_studies_paginated_filtered(0, 10, Some("ACC-SP"))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(studies[0].study_uid, "1.2.3.4.spine");

        // Match by Study UID tail
        let (studies, count) = db
            .get_studies_paginated_filtered(0, 10, Some("chest"))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(studies[0].study_uid, "1.2.3.4.chest");

        // Whitespace-only search degrades to no filter
        let (_studies, count) = db
            .get_studies_paginated_filtered(0, 10, Some("   "))
            .await
            .unwrap();
        assert_eq!(count, 3);

        // None search returns all
        let (_studies, count) = db
            .get_studies_paginated_filtered(0, 10, None)
            .await
            .unwrap();
        assert_eq!(count, 3);

        // No matches
        let (studies, count) = db
            .get_studies_paginated_filtered(0, 10, Some("nonexistent-xyz"))
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert!(studies.is_empty());
    }

    // ---- Upload retry / recovery state machine -------------------------------

    use super::super::database::{attempt_status, study_status};

    /// Build a `Database` while keeping a clonable handle to its pool so tests
    /// can both call the public methods and assert against raw rows. The pool is
    /// an `Arc` internally, so the clone shares the same in-memory database.
    async fn setup_db_with_pool() -> (Database, SqlitePool) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db = Database::new_for_test(pool.clone()).await;
        (db, pool)
    }

    async fn insert_study_with_status(db: &Database, study_uid: &str, status: &str) {
        let study = create_test_study(study_uid);
        db.create_or_update_study(study).await.unwrap();
        // create_or_update_study always defaults to IN-PROGRESS; move it to the
        // status the test needs.
        db.update_study_status(study_uid, status).await.unwrap();
    }

    #[tokio::test]
    async fn test_claim_study_for_upload_is_exclusive() {
        let (db, _pool) = setup_db_with_pool().await;
        insert_study_with_status(&db, "claim.1", study_status::QUEUED).await;

        let first = db
            .claim_study_for_upload("claim.1", &[study_status::QUEUED], "upload-a")
            .await
            .unwrap();
        assert!(first.is_some(), "first claim should succeed");
        let (attempt_id, attempt_no) = first.unwrap();
        assert_eq!(attempt_no, 1);
        assert!(attempt_id > 0);

        // Study is now UPLOADING, so a second claim from QUEUED finds no rows.
        let second = db
            .claim_study_for_upload("claim.1", &[study_status::QUEUED], "upload-b")
            .await
            .unwrap();
        assert!(second.is_none(), "second claim must not double-dispatch");

        let study = db.get_study_by_uid("claim.1").await.unwrap();
        assert_eq!(study.status, study_status::UPLOADING);
        assert_eq!(study.attempts, 1);
        assert!(study.last_attempt_at.is_some());

        let attempts = db.upload_attempts_for_study("claim.1").await.unwrap();
        assert_eq!(attempts.len(), 1, "exactly one STARTED attempt row");
        assert_eq!(attempts[0].status, attempt_status::STARTED);
        assert_eq!(attempts[0].attempt_no, 1);
    }

    #[tokio::test]
    async fn test_mark_attempt_success_closes_attempt_row() {
        let (db, _pool) = setup_db_with_pool().await;
        insert_study_with_status(&db, "ok.1", study_status::QUEUED).await;

        let (attempt_id, _) = db
            .claim_study_for_upload("ok.1", &[study_status::QUEUED], "upload-a")
            .await
            .unwrap()
            .unwrap();

        db.mark_attempt_success(attempt_id).await.unwrap();

        let attempts = db.upload_attempts_for_study("ok.1").await.unwrap();
        assert_eq!(attempts[0].status, attempt_status::SUCCESS);
        assert!(attempts[0].finished_at.is_some());
    }

    #[tokio::test]
    async fn test_mark_upload_failed_retrying_then_terminal() {
        let (db, _pool) = setup_db_with_pool().await;
        insert_study_with_status(&db, "fail.1", study_status::QUEUED).await;

        // First failure with a retry window -> RETRYING.
        let (attempt_id, _) = db
            .claim_study_for_upload("fail.1", &[study_status::QUEUED], "upload-a")
            .await
            .unwrap()
            .unwrap();
        let next = Utc::now() + chrono::Duration::seconds(30);
        db.mark_upload_failed(attempt_id, "fail.1", "boom", Some(next))
            .await
            .unwrap();

        let study = db.get_study_by_uid("fail.1").await.unwrap();
        assert_eq!(study.status, study_status::RETRYING);
        assert_eq!(study.last_error.as_deref(), Some("boom"));
        assert!(study.next_retry_at.is_some());

        // Exhausted: no retry window -> terminal FAILED.
        let (attempt_id2, _) = db
            .claim_study_for_upload("fail.1", &[study_status::RETRYING], "upload-b")
            .await
            .unwrap()
            .unwrap();
        db.mark_upload_failed(attempt_id2, "fail.1", "boom again", None)
            .await
            .unwrap();

        let study = db.get_study_by_uid("fail.1").await.unwrap();
        assert_eq!(study.status, study_status::FAILED);
        assert!(study.next_retry_at.is_none());
        assert_eq!(study.attempts, 2);

        let attempts = db.upload_attempts_for_study("fail.1").await.unwrap();
        assert_eq!(attempts.len(), 2);
        assert!(attempts.iter().all(|a| a.status == attempt_status::FAILED));
    }

    #[tokio::test]
    async fn test_find_due_for_retry_respects_next_retry_at() {
        let (db, _pool) = setup_db_with_pool().await;

        insert_study_with_status(&db, "due.queued", study_status::QUEUED).await;

        // RETRYING and already due.
        insert_study_with_status(&db, "due.now", study_status::QUEUED).await;
        let (aid, _) = db
            .claim_study_for_upload("due.now", &[study_status::QUEUED], "u")
            .await
            .unwrap()
            .unwrap();
        db.mark_upload_failed(
            aid,
            "due.now",
            "e",
            Some(Utc::now() - chrono::Duration::seconds(5)),
        )
        .await
        .unwrap();

        // RETRYING but not yet due.
        insert_study_with_status(&db, "due.later", study_status::QUEUED).await;
        let (aid2, _) = db
            .claim_study_for_upload("due.later", &[study_status::QUEUED], "u")
            .await
            .unwrap()
            .unwrap();
        db.mark_upload_failed(
            aid2,
            "due.later",
            "e",
            Some(Utc::now() + chrono::Duration::seconds(600)),
        )
        .await
        .unwrap();

        let due = db.find_due_for_retry(100).await.unwrap();
        assert!(due.contains(&"due.queued".to_string()));
        assert!(due.contains(&"due.now".to_string()));
        assert!(
            !due.contains(&"due.later".to_string()),
            "study whose backoff window has not elapsed must not be returned"
        );
    }

    #[tokio::test]
    async fn test_recover_pending_on_startup() {
        let (db, pool) = setup_db_with_pool().await;

        // Orphaned UPLOADING with a dangling STARTED attempt row.
        insert_study_with_status(&db, "rec.uploading", study_status::QUEUED).await;
        db.claim_study_for_upload("rec.uploading", &[study_status::QUEUED], "u")
            .await
            .unwrap();

        // Fresh IN-PROGRESS (default updated_at = now) must be left alone.
        insert_study_with_status(&db, "rec.fresh", study_status::IN_PROGRESS).await;

        // Stale IN-PROGRESS: seed an old updated_at directly (the AFTER UPDATE
        // trigger only fires on UPDATE, so the INSERT value is preserved).
        sqlx::query(
            "INSERT INTO studies (study_uid, status, images, series_count, updated_at) \
             VALUES (?, ?, 1, 1, datetime('now', '-10 minutes'))",
        )
        .bind("rec.stale")
        .bind(study_status::IN_PROGRESS)
        .execute(&pool)
        .await
        .unwrap();

        let requeued = db.recover_pending_on_startup().await.unwrap();
        assert_eq!(requeued, 2, "UPLOADING + stale IN-PROGRESS are requeued");

        assert_eq!(
            db.get_study_by_uid("rec.uploading").await.unwrap().status,
            study_status::QUEUED
        );
        assert_eq!(
            db.get_study_by_uid("rec.stale").await.unwrap().status,
            study_status::QUEUED
        );
        assert_eq!(
            db.get_study_by_uid("rec.fresh").await.unwrap().status,
            study_status::IN_PROGRESS,
            "a study still being received must not be grabbed"
        );

        // The dangling STARTED row was closed as FAILED.
        let attempts = db.upload_attempts_for_study("rec.uploading").await.unwrap();
        assert_eq!(attempts[0].status, attempt_status::FAILED);
        assert_eq!(attempts[0].error.as_deref(), Some("interrupted by restart"));
    }

    #[tokio::test]
    async fn test_reset_for_manual_retry() {
        let (db, _pool) = setup_db_with_pool().await;

        insert_study_with_status(&db, "man.failed", study_status::QUEUED).await;
        let (aid, _) = db
            .claim_study_for_upload("man.failed", &[study_status::QUEUED], "u")
            .await
            .unwrap()
            .unwrap();
        db.mark_upload_failed(aid, "man.failed", "boom", None)
            .await
            .unwrap();

        db.reset_for_manual_retry("man.failed").await.unwrap();
        let study = db.get_study_by_uid("man.failed").await.unwrap();
        assert_eq!(study.status, study_status::QUEUED);
        assert!(study.last_error.is_none());
        assert!(study.next_retry_at.is_some());
        // History/attempt count preserved as an audit trail.
        assert_eq!(study.attempts, 1);

        // An in-flight upload must not be reset out from under the worker.
        insert_study_with_status(&db, "man.uploading", study_status::QUEUED).await;
        db.claim_study_for_upload("man.uploading", &[study_status::QUEUED], "u")
            .await
            .unwrap();
        db.reset_for_manual_retry("man.uploading").await.unwrap();
        assert_eq!(
            db.get_study_by_uid("man.uploading").await.unwrap().status,
            study_status::UPLOADING
        );
    }

    #[tokio::test]
    async fn test_delete_study_cascades_to_upload_attempts() {
        let (db, _pool) = setup_db_with_pool().await;
        insert_study_with_status(&db, "del.1", study_status::QUEUED).await;
        db.claim_study_for_upload("del.1", &[study_status::QUEUED], "u")
            .await
            .unwrap();
        assert_eq!(
            db.upload_attempts_for_study("del.1").await.unwrap().len(),
            1
        );

        db.delete_study("del.1".to_string()).await.unwrap();
        assert!(
            db.upload_attempts_for_study("del.1")
                .await
                .unwrap()
                .is_empty(),
            "attempt history must be deleted with the study"
        );
    }
}
