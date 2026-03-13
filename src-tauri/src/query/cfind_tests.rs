#[cfg(test)]
mod tests {
    use crate::query::cfind::*;
    use crate::query::models::{PacsService, QueryFilters};
    use dicom::core::{DataElement, Tag, VR};
    use dicom::dicom_value;
    use dicom::dictionary_std::tags;
    use dicom::encoding::TransferSyntaxIndex;
    use dicom::object::InMemDicomObject;

    // =======================================================================
    // Helper macro for brevity: get Implicit VR LE transfer syntax (erased)
    // =======================================================================
    macro_rules! ivr_le {
        () => {
            dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased()
        };
    }

    // =======================================================================
    // build_cfind_command tests
    // =======================================================================

    #[test]
    fn test_build_cfind_command_fields() {
        let cmd = build_cfind_command(42);

        // AffectedSOPClassUID should be Study Root FIND
        let sop_uid = cmd
            .element(tags::AFFECTED_SOP_CLASS_UID)
            .expect("missing AffectedSOPClassUID");
        assert_eq!(
            sop_uid.to_str().unwrap().trim_end_matches('\0'),
            "1.2.840.10008.5.1.4.1.2.2.1"
        );

        // CommandField should be 0x0020 (C-FIND-RQ)
        let command_field: u16 = cmd
            .element(tags::COMMAND_FIELD)
            .expect("missing CommandField")
            .to_int()
            .unwrap();
        assert_eq!(command_field, 0x0020);

        // MessageID should be 42
        let msg_id: u16 = cmd
            .element(tags::MESSAGE_ID)
            .expect("missing MessageID")
            .to_int()
            .unwrap();
        assert_eq!(msg_id, 42);

        // Priority should be 0x0000 (medium)
        let priority: u16 = cmd
            .element(tags::PRIORITY)
            .expect("missing Priority")
            .to_int()
            .unwrap();
        assert_eq!(priority, 0x0000);

        // CommandDataSetType should be 0x0001 (dataset present; 0x0101 = no dataset)
        let ds_type: u16 = cmd
            .element(tags::COMMAND_DATA_SET_TYPE)
            .expect("missing CommandDataSetType")
            .to_int()
            .unwrap();
        assert_eq!(ds_type, 0x0001);
    }

    #[test]
    fn test_build_cfind_command_different_message_ids() {
        for id in [1u16, 100, 0xFFFF] {
            let cmd = build_cfind_command(id);
            let msg_id: u16 = cmd.element(tags::MESSAGE_ID).unwrap().to_int().unwrap();
            assert_eq!(msg_id, id);
        }
    }

    #[test]
    fn test_build_cfind_command_serializes() {
        let cmd = build_cfind_command(1);
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        cmd.write_dataset_with_ts(&mut bytes, &ts)
            .expect("Failed to serialize command");
        assert!(!bytes.is_empty());
    }

    // =======================================================================
    // build_cfind_identifier tests - STUDY level
    // =======================================================================

    #[test]
    fn test_build_cfind_identifier_study_level_empty_filters() {
        let filters = QueryFilters::default();
        let ident = build_cfind_identifier("STUDY", &filters);

        // QueryRetrieveLevel must be "STUDY"
        let qr_level = ident
            .element(tags::QUERY_RETRIEVE_LEVEL)
            .expect("missing QueryRetrieveLevel");
        assert_eq!(qr_level.to_str().unwrap().trim(), "STUDY");

        // StudyInstanceUID should be present (as empty return key)
        assert!(ident.element(tags::STUDY_INSTANCE_UID).is_ok());

        // PatientName should be present but empty (return key)
        let pn = ident
            .element(tags::PATIENT_NAME)
            .expect("missing PatientName");
        let pn_str = pn.to_str().unwrap_or_default();
        assert!(pn_str.is_empty() || pn_str.trim().is_empty());
    }

    #[test]
    fn test_build_cfind_identifier_study_level_with_patient_name_filter() {
        let filters = QueryFilters {
            patient_name: Some("DOE^JOHN".to_string()),
            ..Default::default()
        };
        let ident = build_cfind_identifier("STUDY", &filters);

        let pn = ident
            .element(tags::PATIENT_NAME)
            .expect("missing PatientName");
        assert_eq!(pn.to_str().unwrap().trim_end_matches('\0'), "DOE^JOHN");
    }

    #[test]
    fn test_build_cfind_identifier_study_level_with_study_date_filter() {
        let filters = QueryFilters {
            study_date: Some("20240101-20241231".to_string()),
            ..Default::default()
        };
        let ident = build_cfind_identifier("STUDY", &filters);

        let sd = ident.element(tags::STUDY_DATE).expect("missing StudyDate");
        assert_eq!(
            sd.to_str().unwrap().trim_end_matches('\0'),
            "20240101-20241231"
        );
    }

    #[test]
    fn test_build_cfind_identifier_study_level_with_modality_filter() {
        let filters = QueryFilters {
            modality: Some("CT".to_string()),
            ..Default::default()
        };
        let ident = build_cfind_identifier("STUDY", &filters);

        let mod_tag = ident
            .element(tags::MODALITIES_IN_STUDY)
            .expect("missing ModalitiesInStudy");
        assert_eq!(mod_tag.to_str().unwrap().trim_end_matches('\0'), "CT");
    }

    #[test]
    fn test_build_cfind_identifier_study_level_with_all_filters() {
        let filters = QueryFilters {
            patient_name: Some("SMITH*".to_string()),
            patient_id: Some("PID001".to_string()),
            study_date: Some("20230601".to_string()),
            accession_number: Some("ACC123".to_string()),
            modality: Some("MR".to_string()),
            study_instance_uid: None,
        };
        let ident = build_cfind_identifier("STUDY", &filters);

        assert_eq!(
            ident
                .element(tags::PATIENT_NAME)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches('\0'),
            "SMITH*"
        );
        assert_eq!(
            ident
                .element(tags::PATIENT_ID)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches('\0'),
            "PID001"
        );
        assert_eq!(
            ident
                .element(tags::STUDY_DATE)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches('\0'),
            "20230601"
        );
        assert_eq!(
            ident
                .element(tags::ACCESSION_NUMBER)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches('\0'),
            "ACC123"
        );
        assert_eq!(
            ident
                .element(tags::MODALITIES_IN_STUDY)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches('\0'),
            "MR"
        );
    }

    #[test]
    fn test_build_cfind_identifier_study_level_has_return_keys() {
        let filters = QueryFilters::default();
        let ident = build_cfind_identifier("STUDY", &filters);

        // These should all be present as return keys (empty values)
        assert!(ident.element(tags::STUDY_TIME).is_ok());
        assert!(ident.element(tags::STUDY_DESCRIPTION).is_ok());
        assert!(ident.element(tags::STUDY_INSTANCE_UID).is_ok());
        // PatientBirthDate (0010,0030)
        assert!(ident.element(Tag(0x0010, 0x0030)).is_ok());
        // PatientSex (0010,0040)
        assert!(ident.element(Tag(0x0010, 0x0040)).is_ok());
        // StudyID (0020,0010)
        assert!(ident.element(Tag(0x0020, 0x0010)).is_ok());
        // NumberOfStudyRelatedSeries (0020,1206)
        assert!(ident.element(Tag(0x0020, 0x1206)).is_ok());
        // NumberOfStudyRelatedInstances (0020,1208)
        assert!(ident.element(Tag(0x0020, 0x1208)).is_ok());
    }

    #[test]
    fn test_build_cfind_identifier_study_level_serializes() {
        let filters = QueryFilters {
            patient_name: Some("TEST".to_string()),
            ..Default::default()
        };
        let ident = build_cfind_identifier("STUDY", &filters);
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        ident
            .write_dataset_with_ts(&mut bytes, &ts)
            .expect("Failed to serialize identifier");
        assert!(!bytes.is_empty());
    }

    // =======================================================================
    // build_cfind_identifier tests - PATIENT level
    // =======================================================================

    #[test]
    fn test_build_cfind_identifier_patient_level_sets_correct_qr_level() {
        let filters = QueryFilters::default();
        let ident = build_cfind_identifier("PATIENT", &filters);

        let qr_level = ident
            .element(tags::QUERY_RETRIEVE_LEVEL)
            .expect("missing QueryRetrieveLevel");
        assert_eq!(qr_level.to_str().unwrap().trim(), "PATIENT");
    }

    #[test]
    fn test_build_cfind_identifier_patient_level_has_patient_return_keys() {
        let filters = QueryFilters::default();
        let ident = build_cfind_identifier("PATIENT", &filters);

        // PatientName and PatientID should be present
        assert!(ident.element(tags::PATIENT_NAME).is_ok());
        assert!(ident.element(tags::PATIENT_ID).is_ok());
        // PatientBirthDate (0010,0030)
        assert!(ident.element(Tag(0x0010, 0x0030)).is_ok());
        // PatientSex (0010,0040)
        assert!(ident.element(Tag(0x0010, 0x0040)).is_ok());
        // NumberOfPatientRelatedStudies (0020,1200)
        assert!(ident.element(Tag(0x0020, 0x1200)).is_ok());
    }

    #[test]
    fn test_build_cfind_identifier_patient_level_omits_study_keys() {
        let filters = QueryFilters::default();
        let ident = build_cfind_identifier("PATIENT", &filters);

        // Study-level tags should NOT be present
        assert!(ident.element(tags::STUDY_DATE).is_err());
        assert!(ident.element(tags::STUDY_TIME).is_err());
        assert!(ident.element(tags::STUDY_DESCRIPTION).is_err());
        assert!(ident.element(tags::STUDY_INSTANCE_UID).is_err());
        assert!(ident.element(tags::ACCESSION_NUMBER).is_err());
        assert!(ident.element(tags::MODALITIES_IN_STUDY).is_err());
    }

    #[test]
    fn test_build_cfind_identifier_patient_level_with_name_filter() {
        let filters = QueryFilters {
            patient_name: Some("*ATHU*".to_string()),
            ..Default::default()
        };
        let ident = build_cfind_identifier("PATIENT", &filters);

        let pn = ident
            .element(tags::PATIENT_NAME)
            .expect("missing PatientName");
        assert_eq!(pn.to_str().unwrap().trim_end_matches('\0'), "*ATHU*");
    }

    #[test]
    fn test_build_cfind_identifier_patient_level_serializes() {
        let filters = QueryFilters {
            patient_name: Some("DOE^JANE".to_string()),
            ..Default::default()
        };
        let ident = build_cfind_identifier("PATIENT", &filters);
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        ident
            .write_dataset_with_ts(&mut bytes, &ts)
            .expect("Failed to serialize patient-level identifier");
        assert!(!bytes.is_empty());
    }

    // =======================================================================
    // parse_cfind_result tests - STUDY level
    // =======================================================================

    /// Helper: build a fake STUDY-level C-FIND result dataset and serialize it.
    fn make_study_result_bytes(
        study_uid: &str,
        patient_name: Option<&str>,
        patient_id: Option<&str>,
        study_date: Option<&str>,
        modality: Option<&str>,
        num_series: Option<&str>,
        num_instances: Option<&str>,
    ) -> Vec<u8> {
        let mut elements: Vec<DataElement<InMemDicomObject>> = Vec::new();

        if let Some(v) = study_date {
            elements.push(DataElement::new(
                tags::STUDY_DATE,
                VR::DA,
                dicom_value!(Str, v),
            ));
        }

        elements.push(DataElement::new(
            tags::STUDY_TIME,
            VR::TM,
            dicom_value!(Str, "143022"),
        ));

        elements.push(DataElement::new(
            tags::ACCESSION_NUMBER,
            VR::SH,
            dicom_value!(Str, "ACC001"),
        ));

        if let Some(v) = modality {
            elements.push(DataElement::new(
                tags::MODALITIES_IN_STUDY,
                VR::CS,
                dicom_value!(Str, v),
            ));
        }

        elements.push(DataElement::new(
            tags::STUDY_DESCRIPTION,
            VR::LO,
            dicom_value!(Str, "CT CHEST"),
        ));

        if let Some(v) = patient_name {
            elements.push(DataElement::new(
                tags::PATIENT_NAME,
                VR::PN,
                dicom_value!(Str, v),
            ));
        }

        if let Some(v) = patient_id {
            elements.push(DataElement::new(
                tags::PATIENT_ID,
                VR::LO,
                dicom_value!(Str, v),
            ));
        }

        elements.push(DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, study_uid),
        ));

        if let Some(v) = num_series {
            elements.push(DataElement::new(
                Tag(0x0020, 0x1206),
                VR::IS,
                dicom_value!(Str, v),
            ));
        }

        if let Some(v) = num_instances {
            elements.push(DataElement::new(
                Tag(0x0020, 0x1208),
                VR::IS,
                dicom_value!(Str, v),
            ));
        }

        let obj = InMemDicomObject::from_element_iter(elements);
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        obj.write_dataset_with_ts(&mut bytes, &ts).unwrap();
        bytes
    }

    /// Helper: build a fake PATIENT-level C-FIND result dataset and serialize it.
    fn make_patient_result_bytes(
        patient_name: &str,
        patient_id: &str,
        birth_date: Option<&str>,
        sex: Option<&str>,
        num_studies: Option<&str>,
    ) -> Vec<u8> {
        let mut elements: Vec<DataElement<InMemDicomObject>> = Vec::new();

        elements.push(DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            dicom_value!(Str, patient_name),
        ));

        elements.push(DataElement::new(
            tags::PATIENT_ID,
            VR::LO,
            dicom_value!(Str, patient_id),
        ));

        if let Some(v) = birth_date {
            elements.push(DataElement::new(
                Tag(0x0010, 0x0030),
                VR::DA,
                dicom_value!(Str, v),
            ));
        }

        if let Some(v) = sex {
            elements.push(DataElement::new(
                Tag(0x0010, 0x0040),
                VR::CS,
                dicom_value!(Str, v),
            ));
        }

        if let Some(v) = num_studies {
            elements.push(DataElement::new(
                Tag(0x0020, 0x1200),
                VR::IS,
                dicom_value!(Str, v),
            ));
        }

        let obj = InMemDicomObject::from_element_iter(elements);
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        obj.write_dataset_with_ts(&mut bytes, &ts).unwrap();
        bytes
    }

    #[test]
    fn test_parse_cfind_result_study_level_full() {
        let bytes = make_study_result_bytes(
            "1.2.840.99999",
            Some("DOE^JOHN"),
            Some("12345"),
            Some("20240615"),
            Some("CT"),
            Some("3"),
            Some("245"),
        );

        let ts = ivr_le!();
        let result = parse_cfind_result(&bytes, &ts, "STUDY").expect("failed to parse");
        assert_eq!(result.study_instance_uid.as_deref(), Some("1.2.840.99999"));
        assert_eq!(result.patient_name.as_deref(), Some("DOE^JOHN"));
        assert_eq!(result.patient_id.as_deref(), Some("12345"));
        assert_eq!(result.study_date.as_deref(), Some("20240615"));
        assert_eq!(result.study_time.as_deref(), Some("143022"));
        assert_eq!(result.study_description.as_deref(), Some("CT CHEST"));
        assert_eq!(result.accession_number.as_deref(), Some("ACC001"));
        assert_eq!(result.modalities_in_study.as_deref(), Some("CT"));
        assert_eq!(result.number_of_series, Some(3));
        assert_eq!(result.number_of_instances, Some(245));
    }

    #[test]
    fn test_parse_cfind_result_study_level_minimal() {
        // Only StudyInstanceUID is required for STUDY level
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, "1.2.3.4.5"),
        )]);

        let ts = ivr_le!();
        let mut bytes = Vec::new();
        obj.write_dataset_with_ts(&mut bytes, &ts).unwrap();

        let result = parse_cfind_result(&bytes, &ts, "STUDY").expect("failed to parse");
        assert_eq!(result.study_instance_uid.as_deref(), Some("1.2.3.4.5"));
        assert!(result.patient_name.is_none());
        assert!(result.patient_id.is_none());
        assert!(result.study_date.is_none());
        assert!(result.number_of_series.is_none());
    }

    #[test]
    fn test_parse_cfind_result_study_level_missing_uid_fails() {
        // Dataset with no StudyInstanceUID
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            dicom_value!(Str, "TEST"),
        )]);

        let ts = ivr_le!();
        let mut bytes = Vec::new();
        obj.write_dataset_with_ts(&mut bytes, &ts).unwrap();

        let result = parse_cfind_result(&bytes, &ts, "STUDY");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing StudyInstanceUID"));
    }

    #[test]
    fn test_parse_cfind_result_invalid_bytes_fails() {
        let ts = ivr_le!();
        let result = parse_cfind_result(&[0xFF, 0xFF, 0xFF, 0xFF], &ts, "STUDY");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cfind_result_empty_bytes_fails() {
        let ts = ivr_le!();
        let result = parse_cfind_result(&[], &ts, "STUDY");
        // An empty dataset should either fail to parse or lack StudyInstanceUID
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cfind_result_strips_null_terminators() {
        let obj = InMemDicomObject::from_element_iter(vec![
            DataElement::new(tags::PATIENT_NAME, VR::PN, dicom_value!(Str, "DOE^JOHN\0")),
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, "1.2.3\0"),
            ),
        ]);

        let ts = ivr_le!();
        let mut bytes = Vec::new();
        obj.write_dataset_with_ts(&mut bytes, &ts).unwrap();

        let result = parse_cfind_result(&bytes, &ts, "STUDY").expect("failed to parse");
        assert_eq!(result.study_instance_uid.as_deref(), Some("1.2.3"));
        assert_eq!(result.patient_name.as_deref(), Some("DOE^JOHN"));
    }

    // =======================================================================
    // parse_cfind_result tests - PATIENT level
    // =======================================================================

    #[test]
    fn test_parse_cfind_result_patient_level_full() {
        let bytes = make_patient_result_bytes(
            "ATHUKORALA^Premachandra^^Prof",
            "60.53799",
            Some("19511007"),
            Some("M"),
            Some("5"),
        );

        let ts = ivr_le!();
        let result = parse_cfind_result(&bytes, &ts, "PATIENT").expect("failed to parse");
        assert_eq!(
            result.patient_name.as_deref(),
            Some("ATHUKORALA^Premachandra^^Prof")
        );
        assert_eq!(result.patient_id.as_deref(), Some("60.53799"));
        assert_eq!(result.patient_birth_date.as_deref(), Some("19511007"));
        assert_eq!(result.patient_sex.as_deref(), Some("M"));
        assert_eq!(result.number_of_patient_related_studies, Some(5));
        // Study-level fields should be None
        assert!(result.study_instance_uid.is_none());
        assert!(result.study_date.is_none());
    }

    #[test]
    fn test_parse_cfind_result_patient_level_minimal() {
        // PATIENT-level does NOT require StudyInstanceUID
        let bytes = make_patient_result_bytes("DOE^JOHN", "12345", None, None, None);

        let ts = ivr_le!();
        let result = parse_cfind_result(&bytes, &ts, "PATIENT").expect("failed to parse");
        assert_eq!(result.patient_name.as_deref(), Some("DOE^JOHN"));
        assert_eq!(result.patient_id.as_deref(), Some("12345"));
        assert!(result.patient_birth_date.is_none());
        assert!(result.patient_sex.is_none());
        assert!(result.number_of_patient_related_studies.is_none());
        assert!(result.study_instance_uid.is_none());
    }

    #[test]
    fn test_parse_cfind_result_patient_level_female() {
        let bytes = make_patient_result_bytes(
            "SMITH^JANE",
            "99999",
            Some("19850722"),
            Some("F"),
            Some("3"),
        );

        let ts = ivr_le!();
        let result = parse_cfind_result(&bytes, &ts, "PATIENT").expect("failed to parse");
        assert_eq!(result.patient_sex.as_deref(), Some("F"));
        assert_eq!(result.patient_birth_date.as_deref(), Some("19850722"));
        assert_eq!(result.number_of_patient_related_studies, Some(3));
    }

    // =======================================================================
    // extract_status tests
    // =======================================================================

    fn make_status_obj(status: u16) -> InMemDicomObject {
        InMemDicomObject::command_from_element_iter([DataElement::new(
            tags::STATUS,
            VR::US,
            dicom_value!(U16, [status]),
        )])
    }

    #[test]
    fn test_extract_status_success() {
        let obj = make_status_obj(0x0000);
        assert_eq!(extract_status(&obj).unwrap(), 0x0000);
    }

    #[test]
    fn test_extract_status_pending() {
        let obj = make_status_obj(0xFF00);
        assert_eq!(extract_status(&obj).unwrap(), 0xFF00);
    }

    #[test]
    fn test_extract_status_pending_warning() {
        let obj = make_status_obj(0xFF01);
        assert_eq!(extract_status(&obj).unwrap(), 0xFF01);
    }

    #[test]
    fn test_extract_status_error() {
        let obj = make_status_obj(0xA700);
        assert_eq!(extract_status(&obj).unwrap(), 0xA700);
    }

    #[test]
    fn test_extract_status_missing_tag_fails() {
        let obj = InMemDicomObject::command_from_element_iter([DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x8020]),
        )]);
        let result = extract_status(&obj);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing Status"));
    }

    // =======================================================================
    // extract_string_optional / extract_integer_optional tests
    // =======================================================================

    #[test]
    fn test_extract_string_optional_present() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            dicom_value!(Str, "DOE^JANE"),
        )]);
        assert_eq!(
            extract_string_optional(&obj, tags::PATIENT_NAME),
            Some("DOE^JANE".to_string())
        );
    }

    #[test]
    fn test_extract_string_optional_missing() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            dicom_value!(Str, "X"),
        )]);
        assert!(extract_string_optional(&obj, tags::PATIENT_ID).is_none());
    }

    #[test]
    fn test_extract_string_optional_empty() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            dicom_value!(),
        )]);
        // Empty value should return None
        assert!(extract_string_optional(&obj, tags::PATIENT_NAME).is_none());
    }

    #[test]
    fn test_extract_string_optional_null_padded() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            dicom_value!(Str, "TEST\0\0"),
        )]);
        assert_eq!(
            extract_string_optional(&obj, tags::PATIENT_NAME),
            Some("TEST".to_string())
        );
    }

    #[test]
    fn test_extract_integer_optional_valid() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x0020, 0x1206),
            VR::IS,
            dicom_value!(Str, "42"),
        )]);
        assert_eq!(
            extract_integer_optional(&obj, Tag(0x0020, 0x1206)),
            Some(42)
        );
    }

    #[test]
    fn test_extract_integer_optional_with_whitespace() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x0020, 0x1206),
            VR::IS,
            dicom_value!(Str, " 7 "),
        )]);
        assert_eq!(extract_integer_optional(&obj, Tag(0x0020, 0x1206)), Some(7));
    }

    #[test]
    fn test_extract_integer_optional_missing() {
        let obj = InMemDicomObject::from_element_iter(vec![]);
        assert!(extract_integer_optional(&obj, Tag(0x0020, 0x1206)).is_none());
    }

    #[test]
    fn test_extract_integer_optional_non_numeric() {
        let obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x0020, 0x1206),
            VR::IS,
            dicom_value!(Str, "NaN"),
        )]);
        assert!(extract_integer_optional(&obj, Tag(0x0020, 0x1206)).is_none());
    }

    // =======================================================================
    // execute_cfind integration test - full C-FIND against a mock PACS SCP
    // =======================================================================

    /// Builds a mock C-FIND SCP that accepts an association on the Study Root
    /// FIND abstract syntax, receives the C-FIND-RQ + identifier, and replies
    /// with `num_results` Pending responses followed by a Success.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cfind_against_mock_pacs() {
        use dicom_ul::association::ServerAssociationOptions;
        use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn the mock PACS SCP
        let scp_handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let (stream, _) = listener.accept().await.unwrap();
                let std_stream = stream.into_std().unwrap();
                std_stream.set_nonblocking(false).unwrap();

                // Accept association with Study Root FIND
                let mut options = ServerAssociationOptions::new()
                    .accept_any()
                    .ae_title("MOCK_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options = options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.1");

                let mut association = options.establish(std_stream).unwrap();

                // Commands are always Implicit VR LE per DICOM standard
                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

                // Use the negotiated transfer syntax for data (identifier/results)
                let negotiated_ts_uid = association
                    .presentation_contexts()
                    .first()
                    .map(|pc| pc.transfer_syntax.clone())
                    .unwrap_or_else(|| "1.2.840.10008.1.2".to_string());
                let fallback_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
                let ts = dicom::transfer_syntax::TransferSyntaxRegistry
                    .get(&negotiated_ts_uid)
                    .unwrap_or(&fallback_ts);

                // Receive the C-FIND-RQ command
                let mut pc_id: u8 = 1;
                let mut got_command = false;
                let mut got_data = false;

                // We need to receive both the command PDU and the data PDU
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
                                PDataValueType::Data => {
                                    // partial data fragment, continue
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Send two Pending responses with result datasets

                for i in 0..2 {
                    // Build a result dataset
                    let result_obj = InMemDicomObject::from_element_iter(vec![
                        DataElement::new(
                            tags::PATIENT_NAME,
                            VR::PN,
                            dicom_value!(Str, format!("PATIENT^{}", i).as_str()),
                        ),
                        DataElement::new(
                            tags::PATIENT_ID,
                            VR::LO,
                            dicom_value!(Str, format!("PID{}", i).as_str()),
                        ),
                        DataElement::new(tags::STUDY_DATE, VR::DA, dicom_value!(Str, "20240101")),
                        DataElement::new(
                            tags::STUDY_INSTANCE_UID,
                            VR::UI,
                            dicom_value!(Str, format!("1.2.3.{}", i).as_str()),
                        ),
                    ]);

                    let mut result_bytes = Vec::new();
                    result_obj
                        .write_dataset_with_ts(&mut result_bytes, &ts)
                        .unwrap();

                    // Build Pending C-FIND-RSP command
                    let pending_cmd = InMemDicomObject::command_from_element_iter([
                        DataElement::new(
                            tags::AFFECTED_SOP_CLASS_UID,
                            VR::UI,
                            dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.1"),
                        ),
                        DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8020])), // C-FIND-RSP
                        DataElement::new(
                            tags::MESSAGE_ID_BEING_RESPONDED_TO,
                            VR::US,
                            dicom_value!(U16, [1]),
                        ),
                        DataElement::new(
                            tags::COMMAND_DATA_SET_TYPE,
                            VR::US,
                            dicom_value!(U16, [0x0000]),
                        ), // Dataset present
                        DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0xFF00u16])), // Pending
                    ]);

                    let mut cmd_bytes = Vec::new();
                    pending_cmd
                        .write_dataset_with_ts(&mut cmd_bytes, &command_ts)
                        .unwrap();

                    // Send data first, then command (the standard pattern
                    // for C-FIND: data PDU followed by command PDU per result)
                    let data_pdu = Pdu::PData {
                        data: vec![PDataValue {
                            presentation_context_id: pc_id,
                            value_type: PDataValueType::Data,
                            is_last: true,
                            data: result_bytes,
                        }],
                    };
                    association.send(&data_pdu).unwrap();

                    let cmd_pdu = Pdu::PData {
                        data: vec![PDataValue {
                            presentation_context_id: pc_id,
                            value_type: PDataValueType::Command,
                            is_last: true,
                            data: cmd_bytes,
                        }],
                    };
                    association.send(&cmd_pdu).unwrap();
                }

                // Send final Success C-FIND-RSP (no dataset)
                let success_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.1"),
                    ),
                    DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8020])),
                    DataElement::new(
                        tags::MESSAGE_ID_BEING_RESPONDED_TO,
                        VR::US,
                        dicom_value!(U16, [1]),
                    ),
                    DataElement::new(
                        tags::COMMAND_DATA_SET_TYPE,
                        VR::US,
                        dicom_value!(U16, [0x0101]),
                    ), // No dataset
                    DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0x0000u16])), // Success
                ]);

                let mut cmd_bytes = Vec::new();
                success_cmd
                    .write_dataset_with_ts(&mut cmd_bytes, &command_ts)
                    .unwrap();

                let cmd_pdu = Pdu::PData {
                    data: vec![PDataValue {
                        presentation_context_id: pc_id,
                        value_type: PDataValueType::Command,
                        is_last: true,
                        data: cmd_bytes,
                    }],
                };
                association.send(&cmd_pdu).unwrap();

                // Wait for release
                match association.receive() {
                    Ok(Pdu::ReleaseRQ) => {
                        let _ = association.send(&Pdu::ReleaseRP);
                    }
                    _ => {
                        // Client may have already disconnected
                    }
                }
            });
        });

        // Run the SCU (our C-FIND client)
        let pacs = PacsService {
            ae_title: "MOCK_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let filters = QueryFilters {
            patient_name: Some("PATIENT*".to_string()),
            ..Default::default()
        };

        let results = execute_cfind("BOUNCE", &pacs, "STUDY", &filters)
            .await
            .expect("C-FIND should succeed");

        assert_eq!(results.len(), 2, "Should have received 2 results");

        assert_eq!(results[0].study_instance_uid.as_deref(), Some("1.2.3.0"));
        assert_eq!(results[0].patient_name.as_deref(), Some("PATIENT^0"));
        assert_eq!(results[0].patient_id.as_deref(), Some("PID0"));
        assert_eq!(results[0].study_date.as_deref(), Some("20240101"));

        assert_eq!(results[1].study_instance_uid.as_deref(), Some("1.2.3.1"));
        assert_eq!(results[1].patient_name.as_deref(), Some("PATIENT^1"));
        assert_eq!(results[1].patient_id.as_deref(), Some("PID1"));

        scp_handle.await.unwrap();
    }

    /// Test that execute_cfind returns an error when connecting to a
    /// non-existent host.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cfind_connection_refused() {
        let pacs = PacsService {
            ae_title: "NONEXISTENT".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1, // port 1 is almost certainly not running a DICOM SCP
        };

        let filters = QueryFilters::default();

        let result = execute_cfind("BOUNCE", &pacs, "STUDY", &filters).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Failed to establish") || err.contains("timed out"),
            "Unexpected error: {}",
            err
        );
    }

    /// Test that execute_cfind returns zero results when the mock PACS
    /// immediately sends Success with no Pending responses.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cfind_no_results() {
        use dicom_ul::association::ServerAssociationOptions;
        use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu};
        use tokio::net::TcpListener;

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
                    .ae_title("EMPTY_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options = options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.1");

                let mut association = options.establish(std_stream).unwrap();

                // Commands are always Implicit VR LE per DICOM standard
                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

                // Consume C-FIND-RQ command + data
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

                // Immediately send Success (no results)
                let success_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.1"),
                    ),
                    DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8020])),
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
                    DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0x0000u16])),
                ]);

                let mut cmd_bytes = Vec::new();
                success_cmd
                    .write_dataset_with_ts(&mut cmd_bytes, &command_ts)
                    .unwrap();

                let cmd_pdu = Pdu::PData {
                    data: vec![PDataValue {
                        presentation_context_id: pc_id,
                        value_type: PDataValueType::Command,
                        is_last: true,
                        data: cmd_bytes,
                    }],
                };
                association.send(&cmd_pdu).unwrap();

                if let Ok(Pdu::ReleaseRQ) = association.receive() {
                    let _ = association.send(&Pdu::ReleaseRP);
                }
            });
        });

        let pacs = PacsService {
            ae_title: "EMPTY_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let results = execute_cfind("BOUNCE", &pacs, "STUDY", &QueryFilters::default())
            .await
            .expect("C-FIND should succeed");

        assert!(results.is_empty(), "Should have zero results");

        scp_handle.await.unwrap();
    }
}
