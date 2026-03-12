#[cfg(test)]
mod tests {
    use crate::aura::query_api::QueryApiClient;
    use crate::query::models::{CfindResult, DicomService, PacsService};
    use dicom::encoding::TransferSyntaxIndex;

    // -----------------------------------------------------------------------
    // Helper: create a QueryApiClient pointed at a mockito server
    // -----------------------------------------------------------------------

    fn make_client(base_url: &str) -> QueryApiClient {
        QueryApiClient::new(base_url.to_string(), "test-api-key".to_string())
    }

    // =======================================================================
    // fetch_services
    // =======================================================================

    #[tokio::test]
    async fn test_fetch_services_success_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/services")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"services":[]}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_services().await.unwrap();

        assert!(resp.services.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_services_success_multiple() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "services": [
                {
                    "id": "svc-1",
                    "label": "Primary PACS",
                    "ae_title": "PRIMARY",
                    "host": "10.0.0.1",
                    "port": 104
                },
                {
                    "id": "svc-2",
                    "label": "Archive",
                    "ae_title": "ARCHIVE",
                    "host": "10.0.0.2",
                    "port": 11112
                }
            ]
        }"#;

        let mock = server
            .mock("GET", "/api/bounce/queries/services")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_services().await.unwrap();

        assert_eq!(resp.services.len(), 2);
        assert_eq!(resp.services[0].id, "svc-1");
        assert_eq!(resp.services[0].label, "Primary PACS");
        assert_eq!(resp.services[0].ae_title, "PRIMARY");
        assert_eq!(resp.services[0].host, "10.0.0.1");
        assert_eq!(resp.services[0].port, 104);
        assert_eq!(resp.services[1].id, "svc-2");
        assert_eq!(resp.services[1].ae_title, "ARCHIVE");
        assert_eq!(resp.services[1].port, 11112);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_services_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/services")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_services().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("401"), "Error should mention 401: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_services_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/services")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_services().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "Error should mention 500: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_services_malformed_json() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/services")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"not_services": true}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_services().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to parse"),
            "Error should mention parsing: {}",
            err
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_services_sends_correct_headers() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/services")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Accept", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"services":[]}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        client.fetch_services().await.unwrap();

        mock.assert_async().await;
    }

    // =======================================================================
    // fetch_pending_queries
    // =======================================================================

    #[tokio::test]
    async fn test_fetch_pending_queries_success_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/pending")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"queries":[]}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_pending_queries().await.unwrap();

        assert!(resp.queries.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_queries_success_with_query() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "queries": [{
                "id": "query-1",
                "service": {
                    "ae_title": "PACS_SCP",
                    "host": "192.168.1.100",
                    "port": 104
                },
                "query_level": "STUDY",
                "filters": {
                    "patient_name": "DOE^JOHN",
                    "modality": "CT"
                }
            }]
        }"#;

        let mock = server
            .mock("GET", "/api/bounce/queries/pending")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_pending_queries().await.unwrap();

        assert_eq!(resp.queries.len(), 1);
        assert_eq!(resp.queries[0].id, "query-1");
        assert_eq!(resp.queries[0].service.ae_title, "PACS_SCP");
        assert_eq!(resp.queries[0].service.host, "192.168.1.100");
        assert_eq!(resp.queries[0].service.port, 104);
        assert_eq!(resp.queries[0].query_level, "STUDY");
        assert_eq!(
            resp.queries[0].filters.patient_name.as_deref(),
            Some("DOE^JOHN")
        );
        assert_eq!(resp.queries[0].filters.modality.as_deref(), Some("CT"));
        assert!(resp.queries[0].filters.study_date.is_none());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_queries_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/pending")
            .with_status(401)
            .with_body(r#"{"message":"Unauthenticated."}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_pending_queries().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("401"), "Error should mention 401: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_queries_malformed_json() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/queries/pending")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"not json at all"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_pending_queries().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to parse"),
            "Error should mention parsing: {}",
            err
        );

        mock.assert_async().await;
    }

    // =======================================================================
    // post_query_results
    // =======================================================================

    #[tokio::test]
    async fn test_post_query_results_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-123/results")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let results = vec![CfindResult {
            patient_name: Some("DOE^JOHN".to_string()),
            patient_id: Some("PID001".to_string()),
            study_date: Some("20240615".to_string()),
            study_time: None,
            study_description: Some("CT CHEST".to_string()),
            accession_number: Some("ACC001".to_string()),
            study_instance_uid: Some("1.2.3.4.5".to_string()),
            modality: None,
            modalities_in_study: Some("CT".to_string()),
            number_of_series: Some(3),
            number_of_instances: Some(100),
            series_instance_uid: None,
            series_description: None,
            series_number: None,
            series_date: None,
            series_time: None,
            body_part_examined: None,
            laterality: None,
            institution_name: None,
            referring_physician_name: None,
            patient_birth_date: None,
            patient_sex: None,
            number_of_patient_related_studies: None,
        }];

        let resp = client.post_query_results("q-123", results).await.unwrap();
        assert_eq!(resp["status"], "ok");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_query_results_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-empty/results")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client
            .post_query_results("q-empty", vec![])
            .await
            .unwrap();
        assert_eq!(resp["status"], "ok");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_query_results_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-fail/results")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.post_query_results("q-fail", vec![]).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "Error should mention 500: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_query_results_sends_correct_payload() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-payload/results")
            .match_header("Accept", "application/json")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"results":[{"study_instance_uid":"1.2.3"}]}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let results = vec![CfindResult {
            patient_name: None,
            patient_id: None,
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            study_instance_uid: Some("1.2.3".to_string()),
            modality: None,
            modalities_in_study: None,
            number_of_series: None,
            number_of_instances: None,
            series_instance_uid: None,
            series_description: None,
            series_number: None,
            series_date: None,
            series_time: None,
            body_part_examined: None,
            laterality: None,
            institution_name: None,
            referring_physician_name: None,
            patient_birth_date: None,
            patient_sex: None,
            number_of_patient_related_studies: None,
        }];

        client
            .post_query_results("q-payload", results)
            .await
            .unwrap();
        mock.assert_async().await;
    }

    // =======================================================================
    // post_query_failed
    // =======================================================================

    #[tokio::test]
    async fn test_post_query_failed_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-err/failed")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client
            .post_query_failed("q-err", "Connection refused".to_string())
            .await
            .unwrap();
        assert_eq!(resp["status"], "ok");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_query_failed_sends_error_payload() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-err2/failed")
            .match_body(mockito::Matcher::JsonString(
                r#"{"error":"DICOM association rejected"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        client
            .post_query_failed("q-err2", "DICOM association rejected".to_string())
            .await
            .unwrap();

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_query_failed_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/q-srv-err/failed")
            .with_status(503)
            .with_body("Service Unavailable")
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client
            .post_query_failed("q-srv-err", "some error".to_string())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("503"), "Error should mention 503: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_query_failed_not_found() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/queries/nonexistent/failed")
            .with_status(404)
            .with_body(r#"{"message":"Query not found"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client
            .post_query_failed("nonexistent", "error".to_string())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("404"), "Error should mention 404: {}", err);

        mock.assert_async().await;
    }

    // =======================================================================
    // fetch_pending_retrieves
    // =======================================================================

    #[tokio::test]
    async fn test_fetch_pending_retrieves_success_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/retrieves/pending")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"retrieves":[]}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_pending_retrieves().await.unwrap();

        assert!(resp.retrieves.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_retrieves_success_with_retrieve() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "retrieves": [{
                "id": "ret-1",
                "service": {
                    "id": "svc-pacs-1",
                    "ae_title": "PACS_SCP",
                    "host": "192.168.1.100",
                    "port": 104
                },
                "study_instance_uid": "1.2.840.113619.2.55.3.123",
                "patient_id": "PID001"
            }]
        }"#;

        let mock = server
            .mock("GET", "/api/bounce/retrieves/pending")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_pending_retrieves().await.unwrap();

        assert_eq!(resp.retrieves.len(), 1);
        assert_eq!(resp.retrieves[0].id, "ret-1");
        assert_eq!(resp.retrieves[0].service.id, "svc-pacs-1");
        assert_eq!(resp.retrieves[0].service.ae_title, "PACS_SCP");
        assert_eq!(resp.retrieves[0].service.host, "192.168.1.100");
        assert_eq!(resp.retrieves[0].service.port, 104);
        assert_eq!(resp.retrieves[0].study_instance_uid, "1.2.840.113619.2.55.3.123");
        assert_eq!(resp.retrieves[0].patient_id.as_deref(), Some("PID001"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_retrieves_multiple() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "retrieves": [
                {
                    "id": "ret-1",
                    "service": {"id": "svc-a", "ae_title": "A", "host": "1.1.1.1", "port": 104},
                    "study_instance_uid": "1.2.3.100",
                    "patient_id": "PID-A"
                },
                {
                    "id": "ret-2",
                    "service": {"id": "svc-b", "ae_title": "B", "host": "2.2.2.2", "port": 11112},
                    "study_instance_uid": "1.2.3.200",
                    "patient_id": null
                }
            ]
        }"#;

        let mock = server
            .mock("GET", "/api/bounce/retrieves/pending")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.fetch_pending_retrieves().await.unwrap();

        assert_eq!(resp.retrieves.len(), 2);
        assert_eq!(resp.retrieves[0].id, "ret-1");
        assert_eq!(resp.retrieves[1].id, "ret-2");
        assert!(resp.retrieves[1].patient_id.is_none());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_retrieves_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/retrieves/pending")
            .with_status(401)
            .with_body(r#"{"message":"Unauthenticated."}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_pending_retrieves().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("401"), "Error should mention 401: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_retrieves_malformed_json() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/retrieves/pending")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"not json at all"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.fetch_pending_retrieves().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to parse"),
            "Error should mention parsing: {}",
            err
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_pending_retrieves_sends_correct_headers() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/bounce/retrieves/pending")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Accept", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"retrieves":[]}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        client.fetch_pending_retrieves().await.unwrap();

        mock.assert_async().await;
    }

    // =======================================================================
    // post_retrieve_completed
    // =======================================================================

    #[tokio::test]
    async fn test_post_retrieve_completed_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-123/completed")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client.post_retrieve_completed("ret-123").await.unwrap();
        assert_eq!(resp["message"], "ok");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_retrieve_completed_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-fail/completed")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.post_retrieve_completed("ret-fail").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "Error should mention 500: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_retrieve_completed_conflict() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-done/completed")
            .with_status(409)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"retrieve already finished"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client.post_retrieve_completed("ret-done").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("409"), "Error should mention 409: {}", err);

        mock.assert_async().await;
    }

    // =======================================================================
    // post_retrieve_failed
    // =======================================================================

    #[tokio::test]
    async fn test_post_retrieve_failed_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-err/failed")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let resp = client
            .post_retrieve_failed("ret-err", "C-MOVE timed out".to_string())
            .await
            .unwrap();
        assert_eq!(resp["message"], "ok");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_retrieve_failed_sends_error_payload() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-err2/failed")
            .match_body(mockito::Matcher::JsonString(
                r#"{"error":"C-MOVE failed with status 0xA701"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"ok"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        client
            .post_retrieve_failed("ret-err2", "C-MOVE failed with status 0xA701".to_string())
            .await
            .unwrap();

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_retrieve_failed_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-srv-err/failed")
            .with_status(503)
            .with_body("Service Unavailable")
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client
            .post_retrieve_failed("ret-srv-err", "some error".to_string())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("503"), "Error should mention 503: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_retrieve_failed_not_found() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/nonexistent/failed")
            .with_status(404)
            .with_body(r#"{"message":"Not Found"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client
            .post_retrieve_failed("nonexistent", "error".to_string())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("404"), "Error should mention 404: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_retrieve_failed_forbidden() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/retrieves/ret-other/failed")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"forbidden"}"#)
            .create_async()
            .await;

        let client = make_client(&server.url());
        let result = client
            .post_retrieve_failed("ret-other", "error".to_string())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("403"), "Error should mention 403: {}", err);

        mock.assert_async().await;
    }

    // =======================================================================
    // find_studies (POST /api/bounce/find)
    // =======================================================================

    #[tokio::test]
    async fn test_find_studies_success_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/find")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"studies":[]}"#)
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        let resp = client
            .find_studies(&FindStudyRequest::default())
            .await
            .unwrap();

        assert!(resp.studies.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_find_studies_success_with_studies() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "studies": [
                {
                    "study_instance_uid": "1.2.840.113619.2.55.3.2831161.0",
                    "study_date": "20240615",
                    "study_time": "143022",
                    "study_description": "CT CHEST WO CONTRAST",
                    "accession_number": "ACC-001",
                    "modalities_in_study": ["CT"],
                    "number_of_series": 3,
                    "number_of_instances": 245,
                    "institution_name": "General Hospital",
                    "referring_physician_name": "DR SMITH",
                    "patient_name": "DOE^JOHN",
                    "patient_id": "PAT-12345",
                    "patient_birth_date": "19800101",
                    "patient_sex": "M"
                }
            ]
        }"#;

        let mock = server
            .mock("POST", "/api/bounce/find")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        let resp = client
            .find_studies(&FindStudyRequest::default())
            .await
            .unwrap();

        assert_eq!(resp.studies.len(), 1);
        let study = &resp.studies[0];
        assert_eq!(
            study.study_instance_uid.as_deref(),
            Some("1.2.840.113619.2.55.3.2831161.0")
        );
        assert_eq!(study.study_date.as_deref(), Some("20240615"));
        assert_eq!(study.study_description.as_deref(), Some("CT CHEST WO CONTRAST"));
        assert_eq!(study.accession_number.as_deref(), Some("ACC-001"));
        assert_eq!(study.patient_name.as_deref(), Some("DOE^JOHN"));
        assert_eq!(study.patient_id.as_deref(), Some("PAT-12345"));
        assert_eq!(study.patient_birth_date.as_deref(), Some("19800101"));
        assert_eq!(study.patient_sex.as_deref(), Some("M"));
        assert_eq!(study.number_of_series, Some(3));
        assert_eq!(study.number_of_instances, Some(245));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_find_studies_sends_filters_in_body() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/find")
            .match_header("Accept", "application/json")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"patient_name":"DOE*","study_date":"20240615"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"studies":[]}"#)
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        client
            .find_studies(&FindStudyRequest {
                patient_name: Some("DOE*".to_string()),
                study_date: Some("20240615".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_find_studies_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/find")
            .with_status(401)
            .with_body(r#"{"message":"Unauthenticated."}"#)
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        let result = client
            .find_studies(&FindStudyRequest::default())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("401"), "Error should mention 401: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_find_studies_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/find")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        let result = client
            .find_studies(&FindStudyRequest::default())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "Error should mention 500: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_find_studies_malformed_json() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/find")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"not_studies": true}"#)
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        let result = client
            .find_studies(&FindStudyRequest::default())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to parse"),
            "Error should mention parsing: {}",
            err
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_find_studies_omits_null_filters_from_body() {
        let mut server = mockito::Server::new_async().await;
        // The body should be {} when all filters are None (no keys at all)
        let mock = server
            .mock("POST", "/api/bounce/find")
            .match_body(mockito::Matcher::JsonString("{}".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"studies":[]}"#)
            .create_async()
            .await;

        use crate::query::models::FindStudyRequest;
        let client = make_client(&server.url());
        client
            .find_studies(&FindStudyRequest::default())
            .await
            .unwrap();

        mock.assert_async().await;
    }

    // =======================================================================
    // Integration: DicomService -> PacsService -> execute_cfind
    // =======================================================================

    /// End-to-end test: fetch DicomService from mock Aurabox, convert to
    /// PacsService, and execute a C-FIND query against a mock PACS SCP.
    ///
    /// This proves that a service fetched from the services endpoint can be
    /// used to run a real DICOM query.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_fetch_service_then_cfind_query() {
        use crate::query::cfind::execute_cfind;
        use crate::query::models::QueryFilters;
        use dicom::core::{DataElement, VR};
        use dicom::dicom_value;
        use dicom::dictionary_std::tags;
        use dicom::object::InMemDicomObject;
        use dicom_ul::association::ServerAssociationOptions;
        use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu};
        use tokio::net::TcpListener;

        // --- 1. Start a mock PACS SCP ---

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let scp_handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let (stream, _) = listener.accept().await.unwrap();
                let std_stream = stream.into_std().unwrap();
                std_stream.set_nonblocking(false).unwrap();

                let mut options = ServerAssociationOptions::new()
                    .accept_any()
                    .ae_title("INTEGRATION_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options =
                    options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.1");

                let mut association = options.establish(std_stream).unwrap();

                // Commands are always Implicit VR LE per DICOM standard
                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                        .erased();

                // Use the negotiated transfer syntax for data (identifier/results)
                let negotiated_ts_uid = association
                    .presentation_contexts()
                    .first()
                    .map(|pc| pc.transfer_syntax.clone())
                    .unwrap_or_else(|| "1.2.840.10008.1.2".to_string());
                let fallback_ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
                let ts = dicom::transfer_syntax::TransferSyntaxRegistry
                    .get(&negotiated_ts_uid)
                    .unwrap_or(&fallback_ts);

                // Receive C-FIND-RQ
                let mut pc_id: u8 = 1;
                let mut got_command = false;
                let mut got_data = false;

                while !got_command || !got_data {
                    let pdu = association.receive().unwrap();
                    if let Pdu::PData { data } = pdu {
                        for dv in &data {
                            match dv.value_type {
                                PDataValueType::Command if dv.is_last => {
                                    pc_id = dv.presentation_context_id;
                                    got_command = true;
                                }
                                PDataValueType::Data if dv.is_last => {
                                    got_data = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Send one Pending result
                let result_obj = InMemDicomObject::from_element_iter(vec![
                    DataElement::new(
                        tags::PATIENT_NAME,
                        VR::PN,
                        dicom_value!(Str, "INTEGRATION^TEST"),
                    ),
                    DataElement::new(
                        tags::PATIENT_ID,
                        VR::LO,
                        dicom_value!(Str, "INT-001"),
                    ),
                    DataElement::new(
                        tags::STUDY_DATE,
                        VR::DA,
                        dicom_value!(Str, "20250101"),
                    ),
                    DataElement::new(
                        tags::STUDY_INSTANCE_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.3.999"),
                    ),
                ]);

                let mut result_bytes = Vec::new();
                result_obj
                    .write_dataset_with_ts(&mut result_bytes, &ts)
                    .unwrap();

                let pending_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.1"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8020]),
                    ),
                    DataElement::new(
                        tags::MESSAGE_ID_BEING_RESPONDED_TO,
                        VR::US,
                        dicom_value!(U16, [1]),
                    ),
                    DataElement::new(
                        tags::COMMAND_DATA_SET_TYPE,
                        VR::US,
                        dicom_value!(U16, [0x0000]),
                    ),
                    DataElement::new(
                        tags::STATUS,
                        VR::US,
                        dicom_value!(U16, [0xFF00u16]),
                    ),
                ]);

                let mut cmd_bytes = Vec::new();
                pending_cmd
                    .write_dataset_with_ts(&mut cmd_bytes, &command_ts)
                    .unwrap();

                association
                    .send(&Pdu::PData {
                        data: vec![PDataValue {
                            presentation_context_id: pc_id,
                            value_type: PDataValueType::Data,
                            is_last: true,
                            data: result_bytes,
                        }],
                    })
                    .unwrap();

                association
                    .send(&Pdu::PData {
                        data: vec![PDataValue {
                            presentation_context_id: pc_id,
                            value_type: PDataValueType::Command,
                            is_last: true,
                            data: cmd_bytes,
                        }],
                    })
                    .unwrap();

                // Send Success
                let success_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.1"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8020]),
                    ),
                    DataElement::new(
                        tags::MESSAGE_ID_BEING_RESPONDED_TO,
                        VR::US,
                        dicom_value!(U16, [1]),
                    ),
                    DataElement::new(
                        tags::COMMAND_DATA_SET_TYPE,
                        VR::US,
                        dicom_value!(U16, [0x0101]),
                    ),
                    DataElement::new(
                        tags::STATUS,
                        VR::US,
                        dicom_value!(U16, [0x0000u16]),
                    ),
                ]);

                let mut cmd_bytes = Vec::new();
                success_cmd
                    .write_dataset_with_ts(&mut cmd_bytes, &command_ts)
                    .unwrap();

                association
                    .send(&Pdu::PData {
                        data: vec![PDataValue {
                            presentation_context_id: pc_id,
                            value_type: PDataValueType::Command,
                            is_last: true,
                            data: cmd_bytes,
                        }],
                    })
                    .unwrap();

                if let Ok(Pdu::ReleaseRQ) = association.receive() {
                    let _ = association.send(&Pdu::ReleaseRP);
                }
            });
        });

        // --- 2. Mock the Aurabox services endpoint ---

        let mut aurabox = mockito::Server::new_async().await;
        let services_body = format!(
            r#"{{
                "services": [{{
                    "id": "svc-integration",
                    "label": "Integration Test PACS",
                    "ae_title": "INTEGRATION_PACS",
                    "host": "{}",
                    "port": {}
                }}]
            }}"#,
            addr.ip(),
            addr.port()
        );

        let mock = aurabox
            .mock("GET", "/api/bounce/queries/services")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&services_body)
            .create_async()
            .await;

        // --- 3. Fetch services from mock Aurabox ---

        let api_client = make_client(&aurabox.url());
        let services_resp = api_client.fetch_services().await.unwrap();

        assert_eq!(services_resp.services.len(), 1);
        let dicom_service: DicomService = services_resp.services[0].clone();
        assert_eq!(dicom_service.id, "svc-integration");
        assert_eq!(dicom_service.label, "Integration Test PACS");
        assert_eq!(dicom_service.ae_title, "INTEGRATION_PACS");

        // --- 4. Convert DicomService -> PacsService ---

        let pacs: PacsService = dicom_service.into();
        assert_eq!(pacs.ae_title, "INTEGRATION_PACS");
        assert_eq!(pacs.host, addr.ip().to_string());
        assert_eq!(pacs.port, addr.port());

        // --- 5. Execute C-FIND against the mock PACS using the converted service ---

        let filters = QueryFilters {
            patient_name: Some("INTEGRATION*".to_string()),
            ..Default::default()
        };

        let results = execute_cfind("BOUNCE_TEST", &pacs, "STUDY", &filters)
            .await
            .expect("C-FIND should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].study_instance_uid.as_deref(), Some("1.2.3.999"));
        assert_eq!(
            results[0].patient_name.as_deref(),
            Some("INTEGRATION^TEST")
        );
        assert_eq!(results[0].patient_id.as_deref(), Some("INT-001"));
        assert_eq!(results[0].study_date.as_deref(), Some("20250101"));

        // --- 6. Verify the mock was called ---

        mock.assert_async().await;
        scp_handle.await.unwrap();
    }
}
