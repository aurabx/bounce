#[cfg(test)]
mod tests {
    use crate::query::models::*;

    // -----------------------------------------------------------------------
    // PacsService
    // -----------------------------------------------------------------------

    #[test]
    fn test_pacs_service_serialize() {
        let service = PacsService {
            ae_title: "PACS_SCP".to_string(),
            host: "192.168.1.100".to_string(),
            port: 104,
        };
        let json = serde_json::to_string(&service).unwrap();
        assert!(json.contains("\"ae_title\":\"PACS_SCP\""));
        assert!(json.contains("\"host\":\"192.168.1.100\""));
        assert!(json.contains("\"port\":104"));
    }

    #[test]
    fn test_pacs_service_deserialize() {
        let json = r#"{"ae_title":"MY_PACS","host":"10.0.0.1","port":11112}"#;
        let service: PacsService = serde_json::from_str(json).unwrap();
        assert_eq!(service.ae_title, "MY_PACS");
        assert_eq!(service.host, "10.0.0.1");
        assert_eq!(service.port, 11112);
    }

    #[test]
    fn test_pacs_service_roundtrip() {
        let original = PacsService {
            ae_title: "TEST".to_string(),
            host: "localhost".to_string(),
            port: 4242,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: PacsService = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ae_title, original.ae_title);
        assert_eq!(deserialized.host, original.host);
        assert_eq!(deserialized.port, original.port);
    }

    // -----------------------------------------------------------------------
    // QueryFilters
    // -----------------------------------------------------------------------

    #[test]
    fn test_query_filters_default_is_all_none() {
        let filters = QueryFilters::default();
        assert!(filters.patient_name.is_none());
        assert!(filters.patient_id.is_none());
        assert!(filters.study_date.is_none());
        assert!(filters.accession_number.is_none());
        assert!(filters.modality.is_none());
    }

    #[test]
    fn test_query_filters_serialize_with_values() {
        let filters = QueryFilters {
            patient_name: Some("DOE^JOHN".to_string()),
            patient_id: None,
            study_date: Some("20240101-20241231".to_string()),
            accession_number: None,
            modality: Some("CT".to_string()),
        };
        let json = serde_json::to_string(&filters).unwrap();
        assert!(json.contains("\"patient_name\":\"DOE^JOHN\""));
        assert!(json.contains("\"patient_id\":null"));
        assert!(json.contains("\"study_date\":\"20240101-20241231\""));
        assert!(json.contains("\"modality\":\"CT\""));
    }

    #[test]
    fn test_query_filters_deserialize_partial() {
        let json = r#"{"patient_name":"SMITH*","modality":"MR"}"#;
        let filters: QueryFilters = serde_json::from_str(json).unwrap();
        assert_eq!(filters.patient_name.as_deref(), Some("SMITH*"));
        assert!(filters.patient_id.is_none());
        assert!(filters.study_date.is_none());
        assert!(filters.accession_number.is_none());
        assert_eq!(filters.modality.as_deref(), Some("MR"));
    }

    #[test]
    fn test_query_filters_deserialize_empty_object() {
        let json = r#"{}"#;
        let filters: QueryFilters = serde_json::from_str(json).unwrap();
        assert!(filters.patient_name.is_none());
        assert!(filters.modality.is_none());
    }

    // -----------------------------------------------------------------------
    // PacsQueryRequest
    // -----------------------------------------------------------------------

    #[test]
    fn test_pacs_query_request_deserialize_full() {
        let json = r#"{
            "id": "abc-123",
            "service": {
                "ae_title": "PACS_SCP",
                "host": "192.168.1.100",
                "port": 104
            },
            "query_level": "STUDY",
            "filters": {
                "patient_name": "DOE^JOHN",
                "study_date": "20240101-20241231",
                "modality": "CT"
            }
        }"#;

        let req: PacsQueryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, "abc-123");
        assert_eq!(req.service.ae_title, "PACS_SCP");
        assert_eq!(req.service.host, "192.168.1.100");
        assert_eq!(req.service.port, 104);
        assert_eq!(req.query_level, "STUDY");
        assert_eq!(req.filters.patient_name.as_deref(), Some("DOE^JOHN"));
        assert_eq!(req.filters.study_date.as_deref(), Some("20240101-20241231"));
        assert_eq!(req.filters.modality.as_deref(), Some("CT"));
        assert!(req.filters.patient_id.is_none());
        assert!(req.filters.accession_number.is_none());
    }

    #[test]
    fn test_pacs_query_request_roundtrip() {
        let original = PacsQueryRequest {
            id: "query-42".to_string(),
            service: PacsService {
                ae_title: "REMOTE".to_string(),
                host: "10.0.0.5".to_string(),
                port: 4242,
            },
            query_level: "STUDY".to_string(),
            filters: QueryFilters {
                patient_name: Some("TEST^PATIENT".to_string()),
                patient_id: Some("PID001".to_string()),
                study_date: None,
                accession_number: Some("ACC999".to_string()),
                modality: None,
            },
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: PacsQueryRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.service.port, original.service.port);
        assert_eq!(
            deserialized.filters.patient_name,
            original.filters.patient_name
        );
        assert_eq!(deserialized.filters.patient_id, original.filters.patient_id);
        assert_eq!(
            deserialized.filters.accession_number,
            original.filters.accession_number
        );
    }

    // -----------------------------------------------------------------------
    // PendingQueriesResponse
    // -----------------------------------------------------------------------

    #[test]
    fn test_pending_queries_response_empty() {
        let json = r#"{"queries":[]}"#;
        let resp: PendingQueriesResponse = serde_json::from_str(json).unwrap();
        assert!(resp.queries.is_empty());
    }

    #[test]
    fn test_pending_queries_response_multiple() {
        let json = r#"{
            "queries": [
                {
                    "id": "q1",
                    "service": {"ae_title": "A", "host": "1.1.1.1", "port": 104},
                    "query_level": "STUDY",
                    "filters": {}
                },
                {
                    "id": "q2",
                    "service": {"ae_title": "B", "host": "2.2.2.2", "port": 11112},
                    "query_level": "STUDY",
                    "filters": {"patient_name": "DOE*"}
                }
            ]
        }"#;

        let resp: PendingQueriesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.queries.len(), 2);
        assert_eq!(resp.queries[0].id, "q1");
        assert_eq!(resp.queries[1].service.ae_title, "B");
        assert_eq!(
            resp.queries[1].filters.patient_name.as_deref(),
            Some("DOE*")
        );
    }

    // -----------------------------------------------------------------------
    // CfindResult
    // -----------------------------------------------------------------------

    #[test]
    fn test_cfind_result_serialize_full() {
        let result = CfindResult {
            patient_name: Some("DOE^JOHN".to_string()),
            patient_id: Some("12345".to_string()),
            study_date: Some("20240615".to_string()),
            study_time: Some("143022".to_string()),
            study_description: Some("CT CHEST W/CONTRAST".to_string()),
            accession_number: Some("ACC001".to_string()),
            study_instance_uid: "1.2.840.113619.2.55.3.123".to_string(),
            modalities_in_study: Some("CT".to_string()),
            number_of_series: Some(3),
            number_of_instances: Some(245),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"patient_name\":\"DOE^JOHN\""));
        assert!(json.contains("\"study_instance_uid\":\"1.2.840.113619.2.55.3.123\""));
        assert!(json.contains("\"number_of_series\":3"));
        assert!(json.contains("\"number_of_instances\":245"));
    }

    #[test]
    fn test_cfind_result_serialize_minimal() {
        let result = CfindResult {
            patient_name: None,
            patient_id: None,
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            study_instance_uid: "1.2.3".to_string(),
            modalities_in_study: None,
            number_of_series: None,
            number_of_instances: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CfindResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.study_instance_uid, "1.2.3");
        assert!(deserialized.patient_name.is_none());
        assert!(deserialized.number_of_series.is_none());
    }

    #[test]
    fn test_cfind_result_roundtrip() {
        let original = CfindResult {
            patient_name: Some("SMITH^JANE".to_string()),
            patient_id: Some("P99".to_string()),
            study_date: Some("20230101".to_string()),
            study_time: None,
            study_description: Some("MR BRAIN".to_string()),
            accession_number: None,
            study_instance_uid: "1.2.840.99999".to_string(),
            modalities_in_study: Some("MR".to_string()),
            number_of_series: Some(5),
            number_of_instances: Some(100),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CfindResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.patient_name, original.patient_name);
        assert_eq!(deserialized.study_instance_uid, original.study_instance_uid);
        assert_eq!(deserialized.number_of_series, original.number_of_series);
    }

    // -----------------------------------------------------------------------
    // QueryResultsPayload
    // -----------------------------------------------------------------------

    #[test]
    fn test_query_results_payload_serialize() {
        let payload = QueryResultsPayload {
            results: vec![
                CfindResult {
                    patient_name: Some("A".to_string()),
                    patient_id: None,
                    study_date: None,
                    study_time: None,
                    study_description: None,
                    accession_number: None,
                    study_instance_uid: "1.2.3".to_string(),
                    modalities_in_study: None,
                    number_of_series: None,
                    number_of_instances: None,
                },
                CfindResult {
                    patient_name: Some("B".to_string()),
                    patient_id: None,
                    study_date: None,
                    study_time: None,
                    study_description: None,
                    accession_number: None,
                    study_instance_uid: "4.5.6".to_string(),
                    modalities_in_study: None,
                    number_of_series: None,
                    number_of_instances: None,
                },
            ],
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"results\":["));
        let deserialized: QueryResultsPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.results.len(), 2);
    }

    #[test]
    fn test_query_results_payload_empty() {
        let payload = QueryResultsPayload { results: vec![] };
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(json, r#"{"results":[]}"#);
    }

    // -----------------------------------------------------------------------
    // DicomService
    // -----------------------------------------------------------------------

    #[test]
    fn test_dicom_service_serialize() {
        let service = DicomService {
            id: "9d97c426-1234-5678-abcd-ef0123456789".to_string(),
            label: "Remote PACS".to_string(),
            ae_title: "MY_PACS".to_string(),
            host: "127.0.0.1".to_string(),
            port: 104,
        };
        let json = serde_json::to_string(&service).unwrap();
        assert!(json.contains("\"id\":\"9d97c426-1234-5678-abcd-ef0123456789\""));
        assert!(json.contains("\"label\":\"Remote PACS\""));
        assert!(json.contains("\"ae_title\":\"MY_PACS\""));
        assert!(json.contains("\"host\":\"127.0.0.1\""));
        assert!(json.contains("\"port\":104"));
    }

    #[test]
    fn test_dicom_service_deserialize() {
        let json = r#"{
            "id": "abc-def-123",
            "label": "Test PACS",
            "ae_title": "TEST_SCP",
            "host": "10.0.0.50",
            "port": 11112
        }"#;
        let service: DicomService = serde_json::from_str(json).unwrap();
        assert_eq!(service.id, "abc-def-123");
        assert_eq!(service.label, "Test PACS");
        assert_eq!(service.ae_title, "TEST_SCP");
        assert_eq!(service.host, "10.0.0.50");
        assert_eq!(service.port, 11112);
    }

    #[test]
    fn test_dicom_service_roundtrip() {
        let original = DicomService {
            id: "svc-42".to_string(),
            label: "Main Archive".to_string(),
            ae_title: "ARCHIVE".to_string(),
            host: "pacs.local".to_string(),
            port: 4242,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DicomService = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_dicom_service_equality() {
        let a = DicomService {
            id: "id-1".to_string(),
            label: "PACS A".to_string(),
            ae_title: "AE_A".to_string(),
            host: "1.1.1.1".to_string(),
            port: 104,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_dicom_service_inequality() {
        let a = DicomService {
            id: "id-1".to_string(),
            label: "PACS A".to_string(),
            ae_title: "AE_A".to_string(),
            host: "1.1.1.1".to_string(),
            port: 104,
        };
        let b = DicomService {
            id: "id-2".to_string(),
            label: "PACS B".to_string(),
            ae_title: "AE_B".to_string(),
            host: "2.2.2.2".to_string(),
            port: 11112,
        };
        assert_ne!(a, b);
    }

    // -----------------------------------------------------------------------
    // DicomService -> PacsService conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_dicom_service_to_pacs_service() {
        let dicom_svc = DicomService {
            id: "svc-99".to_string(),
            label: "Hospital PACS".to_string(),
            ae_title: "HOSP_PACS".to_string(),
            host: "192.168.1.200".to_string(),
            port: 104,
        };
        let pacs: PacsService = dicom_svc.into();
        assert_eq!(pacs.ae_title, "HOSP_PACS");
        assert_eq!(pacs.host, "192.168.1.200");
        assert_eq!(pacs.port, 104);
    }

    #[test]
    fn test_dicom_service_to_pacs_service_discards_id_and_label() {
        let dicom_svc = DicomService {
            id: "should-be-gone".to_string(),
            label: "also-gone".to_string(),
            ae_title: "KEEP_ME".to_string(),
            host: "keep.this.host".to_string(),
            port: 5555,
        };
        let pacs: PacsService = PacsService::from(dicom_svc);
        assert_eq!(pacs.ae_title, "KEEP_ME");
        assert_eq!(pacs.host, "keep.this.host");
        assert_eq!(pacs.port, 5555);
        // PacsService has no id or label fields — conversion is lossy by design
    }

    // -----------------------------------------------------------------------
    // ServicesResponse
    // -----------------------------------------------------------------------

    #[test]
    fn test_services_response_empty() {
        let json = r#"{"services":[]}"#;
        let resp: ServicesResponse = serde_json::from_str(json).unwrap();
        assert!(resp.services.is_empty());
    }

    #[test]
    fn test_services_response_single() {
        let json = r#"{
            "services": [{
                "id": "svc-1",
                "label": "Primary PACS",
                "ae_title": "PRIMARY",
                "host": "10.0.0.1",
                "port": 104
            }]
        }"#;
        let resp: ServicesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.services.len(), 1);
        assert_eq!(resp.services[0].id, "svc-1");
        assert_eq!(resp.services[0].label, "Primary PACS");
        assert_eq!(resp.services[0].ae_title, "PRIMARY");
    }

    #[test]
    fn test_services_response_multiple() {
        let json = r#"{
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
                    "label": "Archive PACS",
                    "ae_title": "ARCHIVE",
                    "host": "10.0.0.2",
                    "port": 11112
                },
                {
                    "id": "svc-3",
                    "label": "Research PACS",
                    "ae_title": "RESEARCH",
                    "host": "10.0.0.3",
                    "port": 4242
                }
            ]
        }"#;
        let resp: ServicesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.services.len(), 3);
        assert_eq!(resp.services[0].label, "Primary PACS");
        assert_eq!(resp.services[1].ae_title, "ARCHIVE");
        assert_eq!(resp.services[2].port, 4242);
    }

    #[test]
    fn test_services_response_roundtrip() {
        let original = ServicesResponse {
            services: vec![
                DicomService {
                    id: "a-1".to_string(),
                    label: "PACS One".to_string(),
                    ae_title: "ONE".to_string(),
                    host: "1.1.1.1".to_string(),
                    port: 104,
                },
                DicomService {
                    id: "b-2".to_string(),
                    label: "PACS Two".to_string(),
                    ae_title: "TWO".to_string(),
                    host: "2.2.2.2".to_string(),
                    port: 11112,
                },
            ],
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ServicesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.services.len(), 2);
        assert_eq!(deserialized.services[0], original.services[0]);
        assert_eq!(deserialized.services[1], original.services[1]);
    }

    #[test]
    fn test_services_response_serialize() {
        let resp = ServicesResponse {
            services: vec![DicomService {
                id: "x-1".to_string(),
                label: "Test".to_string(),
                ae_title: "TEST".to_string(),
                host: "localhost".to_string(),
                port: 104,
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"services\":["));
        assert!(json.contains("\"id\":\"x-1\""));
        assert!(json.contains("\"label\":\"Test\""));
    }

    // -----------------------------------------------------------------------
    // QueryFailedPayload
    // -----------------------------------------------------------------------

    #[test]
    fn test_query_failed_payload_serialize() {
        let payload = QueryFailedPayload {
            error: "Connection refused".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(json, r#"{"error":"Connection refused"}"#);
    }

    #[test]
    fn test_query_failed_payload_roundtrip() {
        let original = QueryFailedPayload {
            error: "Failed to establish DICOM association: connection refused".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: QueryFailedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.error, original.error);
    }
}
