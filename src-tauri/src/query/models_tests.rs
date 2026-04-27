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
            study_instance_uid: None,
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
                study_instance_uid: None,
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
    fn test_cfind_result_serialize_study_level() {
        let result = CfindResult {
            patient_name: Some("DOE^JOHN".to_string()),
            patient_id: Some("12345".to_string()),
            study_date: Some("20240615".to_string()),
            study_time: Some("143022".to_string()),
            study_description: Some("CT CHEST W/CONTRAST".to_string()),
            accession_number: Some("ACC001".to_string()),
            study_instance_uid: Some("1.2.840.113619.2.55.3.123".to_string()),
            modality: None,
            modalities_in_study: Some("CT".to_string()),
            number_of_series: Some(3),
            number_of_instances: Some(245),
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
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"patient_name\":\"DOE^JOHN\""));
        assert!(json.contains("\"study_instance_uid\":\"1.2.840.113619.2.55.3.123\""));
        assert!(json.contains("\"number_of_series\":3"));
        assert!(json.contains("\"number_of_instances\":245"));
        // Patient-level fields should be omitted via skip_serializing_if
        assert!(!json.contains("\"patient_birth_date\""));
        assert!(!json.contains("\"patient_sex\""));
        assert!(!json.contains("\"number_of_patient_related_studies\""));
    }

    #[test]
    fn test_cfind_result_serialize_patient_level() {
        let result = CfindResult {
            patient_name: Some("ATHUKORALA^Premachandra^^Prof".to_string()),
            patient_id: Some("60.53799".to_string()),
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            study_instance_uid: None,
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
            patient_birth_date: Some("19511007".to_string()),
            patient_sex: Some("M".to_string()),
            number_of_patient_related_studies: Some(5),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"patient_birth_date\":\"19511007\""));
        assert!(json.contains("\"patient_sex\":\"M\""));
        assert!(json.contains("\"number_of_patient_related_studies\":5"));
        // study_instance_uid should be omitted
        assert!(!json.contains("\"study_instance_uid\""));
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
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CfindResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.study_instance_uid.as_deref(), Some("1.2.3"));
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
            study_instance_uid: Some("1.2.840.99999".to_string()),
            modality: None,
            modalities_in_study: Some("MR".to_string()),
            number_of_series: Some(5),
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
    fn test_query_results_payload_serialize_study_level() {
        let payload = QueryResultsPayload {
            results: vec![
                CfindResult {
                    patient_name: Some("A".to_string()),
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
                },
                CfindResult {
                    patient_name: Some("B".to_string()),
                    patient_id: None,
                    study_date: None,
                    study_time: None,
                    study_description: None,
                    accession_number: None,
                    study_instance_uid: Some("4.5.6".to_string()),
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
                },
            ],
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"results\":["));
        let deserialized: QueryResultsPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.results.len(), 2);
        assert_eq!(
            deserialized.results[0].study_instance_uid.as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            deserialized.results[1].study_instance_uid.as_deref(),
            Some("4.5.6")
        );
    }

    #[test]
    fn test_query_results_payload_serialize_patient_level() {
        let payload = QueryResultsPayload {
            results: vec![CfindResult {
                patient_name: Some("DOE^JOHN".to_string()),
                patient_id: Some("PID001".to_string()),
                study_date: None,
                study_time: None,
                study_description: None,
                accession_number: None,
                study_instance_uid: None,
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
                patient_birth_date: Some("19800115".to_string()),
                patient_sex: Some("M".to_string()),
                number_of_patient_related_studies: Some(3),
            }],
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"patient_birth_date\":\"19800115\""));
        assert!(json.contains("\"patient_sex\":\"M\""));
        assert!(json.contains("\"number_of_patient_related_studies\":3"));
        // study_instance_uid should be omitted via skip_serializing_if
        assert!(!json.contains("\"study_instance_uid\""));
        let deserialized: QueryResultsPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.results.len(), 1);
        assert_eq!(
            deserialized.results[0].patient_birth_date.as_deref(),
            Some("19800115")
        );
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

    // -----------------------------------------------------------------------
    // RetrieveService
    // -----------------------------------------------------------------------

    #[test]
    fn test_retrieve_service_serialize() {
        let service = RetrieveService {
            id: "svc-pacs-1".to_string(),
            ae_title: "PACS_SCP".to_string(),
            host: "192.168.1.100".to_string(),
            port: 104,
        };
        let json = serde_json::to_string(&service).unwrap();
        assert!(json.contains("\"id\":\"svc-pacs-1\""));
        assert!(json.contains("\"ae_title\":\"PACS_SCP\""));
        assert!(json.contains("\"host\":\"192.168.1.100\""));
        assert!(json.contains("\"port\":104"));
    }

    #[test]
    fn test_retrieve_service_deserialize() {
        let json = r#"{
            "id": "svc-abc",
            "ae_title": "REMOTE_PACS",
            "host": "10.0.0.50",
            "port": 11112
        }"#;
        let service: RetrieveService = serde_json::from_str(json).unwrap();
        assert_eq!(service.id, "svc-abc");
        assert_eq!(service.ae_title, "REMOTE_PACS");
        assert_eq!(service.host, "10.0.0.50");
        assert_eq!(service.port, 11112);
    }

    #[test]
    fn test_retrieve_service_roundtrip() {
        let original = RetrieveService {
            id: "svc-42".to_string(),
            ae_title: "MY_PACS".to_string(),
            host: "pacs.local".to_string(),
            port: 4242,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: RetrieveService = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.ae_title, original.ae_title);
        assert_eq!(deserialized.host, original.host);
        assert_eq!(deserialized.port, original.port);
    }

    // -----------------------------------------------------------------------
    // RetrieveService -> PacsService conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_retrieve_service_to_pacs_service() {
        let retrieve_svc = RetrieveService {
            id: "svc-99".to_string(),
            ae_title: "HOSP_PACS".to_string(),
            host: "192.168.1.200".to_string(),
            port: 104,
        };
        let pacs: PacsService = retrieve_svc.into();
        assert_eq!(pacs.ae_title, "HOSP_PACS");
        assert_eq!(pacs.host, "192.168.1.200");
        assert_eq!(pacs.port, 104);
    }

    #[test]
    fn test_retrieve_service_to_pacs_service_discards_id() {
        let retrieve_svc = RetrieveService {
            id: "should-be-gone".to_string(),
            ae_title: "KEEP_ME".to_string(),
            host: "keep.this.host".to_string(),
            port: 5555,
        };
        let pacs: PacsService = PacsService::from(retrieve_svc);
        assert_eq!(pacs.ae_title, "KEEP_ME");
        assert_eq!(pacs.host, "keep.this.host");
        assert_eq!(pacs.port, 5555);
    }

    // -----------------------------------------------------------------------
    // PendingJobsResponse (unified retrieves + sends, tagged-union)
    // -----------------------------------------------------------------------

    #[test]
    fn test_pending_jobs_response_empty() {
        let json = r#"{"jobs":[]}"#;
        let resp: PendingJobsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.jobs.is_empty());
    }

    #[test]
    fn test_pending_jobs_response_retrieve_variant() {
        let json = r#"{
            "jobs": [{
                "type": "retrieve",
                "id": "ret-1",
                "service": {"id": "svc-a", "ae_title": "A", "host": "1.1.1.1", "port": 104},
                "study_instance_uid": "1.2.3.100",
                "patient_id": "PID-A"
            }]
        }"#;

        let resp: PendingJobsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.jobs.len(), 1);
        match &resp.jobs[0] {
            Job::Retrieve(r) => {
                assert_eq!(r.id, "ret-1");
                assert_eq!(r.service.ae_title, "A");
                assert_eq!(r.study_instance_uid, "1.2.3.100");
                assert_eq!(r.patient_id.as_deref(), Some("PID-A"));
            }
            other => panic!("expected Retrieve variant, got {:?}", other),
        }
    }

    #[test]
    fn test_pending_jobs_response_send_variant() {
        let json = r#"{
            "jobs": [{
                "type": "send",
                "id": "send-9",
                "destination": {"id": "svc-b", "ae_title": "DEST", "host": "10.0.0.5", "port": 104},
                "study_instance_uid": "1.2.3.200",
                "series_uids": ["1.2.3.4.5", "1.2.3.4.6"],
                "source": {
                    "wado_base_url": "https://uhura.example/dicomweb/v3/raw",
                    "study_path": "/studies/1.2.3.200",
                    "jwt": "eyTOKEN"
                }
            }]
        }"#;

        let resp: PendingJobsResponse = serde_json::from_str(json).unwrap();
        match &resp.jobs[0] {
            Job::Send(s) => {
                assert_eq!(s.id, "send-9");
                assert_eq!(s.destination.ae_title, "DEST");
                assert_eq!(s.destination.port, 104);
                assert_eq!(s.study_instance_uid, "1.2.3.200");
                assert_eq!(
                    s.series_uids.as_deref(),
                    Some(&["1.2.3.4.5".to_string(), "1.2.3.4.6".to_string()][..])
                );
                assert_eq!(s.source.wado_base_url, "https://uhura.example/dicomweb/v3/raw");
                assert_eq!(s.source.study_path, "/studies/1.2.3.200");
                assert_eq!(s.source.jwt, "eyTOKEN");
            }
            other => panic!("expected Send variant, got {:?}", other),
        }
    }

    #[test]
    fn test_pending_jobs_response_send_with_null_series() {
        let json = r#"{
            "jobs": [{
                "type": "send",
                "id": "send-all",
                "destination": {"id": "d1", "ae_title": "D", "host": "x", "port": 4242},
                "study_instance_uid": "1.2.3.300",
                "series_uids": null,
                "source": {"wado_base_url": "https://u/", "study_path": "/s/x", "jwt": "j"}
            }]
        }"#;

        let resp: PendingJobsResponse = serde_json::from_str(json).unwrap();
        match &resp.jobs[0] {
            Job::Send(s) => assert!(s.series_uids.is_none()),
            other => panic!("expected Send variant, got {:?}", other),
        }
    }

    #[test]
    fn test_pending_jobs_response_mixed() {
        let json = r#"{
            "jobs": [
                {
                    "type": "retrieve",
                    "id": "r-1",
                    "service": {"id": "svc-a", "ae_title": "A", "host": "1.1.1.1", "port": 104},
                    "study_instance_uid": "1.2.3.r",
                    "patient_id": null
                },
                {
                    "type": "send",
                    "id": "s-1",
                    "destination": {"id": "svc-b", "ae_title": "B", "host": "2.2.2.2", "port": 104},
                    "study_instance_uid": "1.2.3.s",
                    "series_uids": null,
                    "source": {"wado_base_url": "https://u", "study_path": "/p", "jwt": "j"}
                }
            ]
        }"#;

        let resp: PendingJobsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.jobs.len(), 2);
        assert!(matches!(resp.jobs[0], Job::Retrieve(_)));
        assert!(matches!(resp.jobs[1], Job::Send(_)));
    }

    #[test]
    fn test_send_progress_payload_omits_null_instance_count() {
        let payload = SendProgressPayload {
            instances_sent: 5,
            instance_count: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"instances_sent\":5"));
        assert!(!json.contains("instance_count"));
    }

    #[test]
    fn test_send_progress_payload_includes_instance_count_when_set() {
        let payload = SendProgressPayload {
            instances_sent: 5,
            instance_count: Some(12),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"instance_count\":12"));
    }

    // -----------------------------------------------------------------------
    // RetrieveFailedPayload
    // -----------------------------------------------------------------------

    #[test]
    fn test_retrieve_failed_payload_serialize() {
        let payload = RetrieveFailedPayload {
            error: "C-MOVE failed with status 0xA701".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(json, r#"{"error":"C-MOVE failed with status 0xA701"}"#);
    }

    #[test]
    fn test_retrieve_failed_payload_roundtrip() {
        let original = RetrieveFailedPayload {
            error: "C-MOVE timed out after 300s".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: RetrieveFailedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.error, original.error);
    }
}
