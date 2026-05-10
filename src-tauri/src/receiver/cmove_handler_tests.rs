#[cfg(test)]
mod tests {
    use crate::aura::query_api::{MoveResolveOutcome, QueryApiClient};
    use crate::query::models::MoveResolveRequest;

    /// The Aura-side `move/resolve` endpoint returns three success-shaped
    /// outcomes that the SCP handler maps to different DIMSE status codes.
    /// These tests ensure the QueryApiClient correctly classifies each.
    #[tokio::test]
    async fn test_resolve_move_returns_resolved_for_success_response() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "source": {
                "study_id": "uuid-123",
                "wado_base_url": "https://uhura/dicomweb/v3/raw",
                "study_path": "/studies/1.2.3",
                "jwt": "TOKEN"
            },
            "destination": {
                "id": "svc-9",
                "ae_title": "WORKSTATION",
                "host": "10.0.0.1",
                "port": 11112
            }
        }"#;

        let mock = server
            .mock("POST", "/api/bounce/move/resolve")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = QueryApiClient::new(server.url(), "test-key".into());
        let request = MoveResolveRequest {
            study_instance_uid: "1.2.3".into(),
            move_destination_ae: Some("WORKSTATION".into()),
        };

        let outcome = client.resolve_move(&request).await.unwrap();

        match outcome {
            MoveResolveOutcome::Resolved(r) => {
                assert_eq!(r.source.study_id.as_deref(), Some("uuid-123"));
                assert_eq!(r.source.jwt, "TOKEN");
                let dest = r.destination.expect("destination present");
                assert_eq!(dest.ae_title, "WORKSTATION");
                assert_eq!(dest.port, 11112);
            }
            other => panic!("expected Resolved, got {:?}", other),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_move_returns_not_found_on_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/move/resolve")
            .with_status(404)
            .with_body(r#"{"message":"study not viewable in this realm"}"#)
            .create_async()
            .await;

        let client = QueryApiClient::new(server.url(), "test-key".into());
        let request = MoveResolveRequest {
            study_instance_uid: "missing".into(),
            move_destination_ae: None,
        };

        let outcome = client.resolve_move(&request).await.unwrap();
        assert!(matches!(outcome, MoveResolveOutcome::NotFound));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_move_returns_destination_unknown_on_422() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/move/resolve")
            .with_status(422)
            .with_body(r#"{"message":"move destination AE not configured"}"#)
            .create_async()
            .await;

        let client = QueryApiClient::new(server.url(), "test-key".into());
        let request = MoveResolveRequest {
            study_instance_uid: "1.2.3".into(),
            move_destination_ae: Some("UNKNOWN_AE".into()),
        };

        let outcome = client.resolve_move(&request).await.unwrap();
        assert!(matches!(outcome, MoveResolveOutcome::DestinationUnknown));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_move_returns_error_on_500() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/bounce/move/resolve")
            .with_status(500)
            .with_body("internal error")
            .create_async()
            .await;

        let client = QueryApiClient::new(server.url(), "test-key".into());
        let request = MoveResolveRequest {
            study_instance_uid: "1.2.3".into(),
            move_destination_ae: None,
        };

        let result = client.resolve_move(&request).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "expected 500 in error: {}", err);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_move_resolved_without_destination() {
        // Source-only response (e.g. a future C-GET path that doesn't ask
        // Aura to resolve a destination AE).
        let mut server = mockito::Server::new_async().await;
        let body = r#"{
            "source": {
                "study_id": "uuid-1",
                "wado_base_url": "https://uhura",
                "study_path": "/studies/1",
                "jwt": "T"
            }
        }"#;

        let mock = server
            .mock("POST", "/api/bounce/move/resolve")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = QueryApiClient::new(server.url(), "test-key".into());
        let request = MoveResolveRequest {
            study_instance_uid: "1".into(),
            move_destination_ae: None,
        };

        let outcome = client.resolve_move(&request).await.unwrap();
        match outcome {
            MoveResolveOutcome::Resolved(r) => {
                assert!(r.destination.is_none());
                assert_eq!(r.source.jwt, "T");
            }
            other => panic!("expected Resolved, got {:?}", other),
        }

        mock.assert_async().await;
    }
}
