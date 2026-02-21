#[cfg(test)]
mod tests {
    use crate::query::cmove::*;
    use crate::query::models::PacsService;
    use dicom::core::{DataElement, Tag, VR};
    use dicom::dicom_value;
    use dicom::dictionary_std::tags;
    use dicom::object::InMemDicomObject;

    // =======================================================================
    // Helper macro: get Implicit VR LE transfer syntax (erased)
    // =======================================================================
    macro_rules! ivr_le {
        () => {
            dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased()
        };
    }

    // =======================================================================
    // build_cmove_command tests
    // =======================================================================

    #[test]
    fn test_build_cmove_command_fields() {
        let cmd = build_cmove_command(1, "BOUNCE_SCP");

        // AffectedSOPClassUID should be Study Root MOVE
        let sop_uid = cmd
            .element(tags::AFFECTED_SOP_CLASS_UID)
            .expect("missing AffectedSOPClassUID");
        assert_eq!(
            sop_uid.to_str().unwrap().trim_end_matches('\0'),
            "1.2.840.10008.5.1.4.1.2.2.2"
        );

        // CommandField should be 0x0021 (C-MOVE-RQ)
        let command_field: u16 = cmd
            .element(tags::COMMAND_FIELD)
            .expect("missing CommandField")
            .to_int()
            .unwrap();
        assert_eq!(command_field, 0x0021);

        // MessageID should be 1
        let msg_id: u16 = cmd
            .element(tags::MESSAGE_ID)
            .expect("missing MessageID")
            .to_int()
            .unwrap();
        assert_eq!(msg_id, 1);

        // Priority should be 0x0000 (medium)
        let priority: u16 = cmd
            .element(tags::PRIORITY)
            .expect("missing Priority")
            .to_int()
            .unwrap();
        assert_eq!(priority, 0x0000);

        // CommandDataSetType should be 0x0001 (dataset present)
        let ds_type: u16 = cmd
            .element(tags::COMMAND_DATA_SET_TYPE)
            .expect("missing CommandDataSetType")
            .to_int()
            .unwrap();
        assert_eq!(ds_type, 0x0001);

        // MoveDestination (0000,0600) should be "BOUNCE_SCP"
        let move_dest_tag = Tag(0x0000, 0x0600);
        let move_dest = cmd
            .element(move_dest_tag)
            .expect("missing MoveDestination");
        assert_eq!(
            move_dest.to_str().unwrap().trim_end_matches('\0').trim(),
            "BOUNCE_SCP"
        );
    }

    #[test]
    fn test_build_cmove_command_different_message_ids() {
        for id in [1u16, 100, 0xFFFF] {
            let cmd = build_cmove_command(id, "DEST");
            let msg_id: u16 = cmd
                .element(tags::MESSAGE_ID)
                .unwrap()
                .to_int()
                .unwrap();
            assert_eq!(msg_id, id);
        }
    }

    #[test]
    fn test_build_cmove_command_different_destinations() {
        let move_dest_tag = Tag(0x0000, 0x0600);

        for dest in ["BOUNCE", "MY_SCP", "LONG_AE_TITLE_16"] {
            let cmd = build_cmove_command(1, dest);
            let move_dest = cmd.element(move_dest_tag).unwrap();
            assert_eq!(
                move_dest.to_str().unwrap().trim_end_matches('\0').trim(),
                dest
            );
        }
    }

    #[test]
    fn test_build_cmove_command_serializes() {
        let cmd = build_cmove_command(1, "BOUNCE");
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        cmd.write_dataset_with_ts(&mut bytes, &ts)
            .expect("Failed to serialize C-MOVE command");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_build_cmove_command_differs_from_cfind() {
        use crate::query::cfind::build_cfind_command;

        let cmove_cmd = build_cmove_command(1, "BOUNCE");
        let cfind_cmd = build_cfind_command(1);

        // CommandField: C-MOVE-RQ = 0x0021, C-FIND-RQ = 0x0020
        let cmove_field: u16 = cmove_cmd
            .element(tags::COMMAND_FIELD)
            .unwrap()
            .to_int()
            .unwrap();
        let cfind_field: u16 = cfind_cmd
            .element(tags::COMMAND_FIELD)
            .unwrap()
            .to_int()
            .unwrap();
        assert_eq!(cmove_field, 0x0021);
        assert_eq!(cfind_field, 0x0020);
        assert_ne!(cmove_field, cfind_field);

        // AffectedSOPClassUID: MOVE vs FIND
        let cmove_sop = cmove_cmd
            .element(tags::AFFECTED_SOP_CLASS_UID)
            .unwrap()
            .to_str()
            .unwrap()
            .trim_end_matches('\0')
            .to_string();
        let cfind_sop = cfind_cmd
            .element(tags::AFFECTED_SOP_CLASS_UID)
            .unwrap()
            .to_str()
            .unwrap()
            .trim_end_matches('\0')
            .to_string();
        assert_ne!(cmove_sop, cfind_sop);
        assert!(cmove_sop.ends_with(".2")); // ...2.2.2 (MOVE)
        assert!(cfind_sop.ends_with(".1")); // ...2.2.1 (FIND)

        // C-MOVE has MoveDestination, C-FIND does not
        let move_dest_tag = Tag(0x0000, 0x0600);
        assert!(cmove_cmd.element(move_dest_tag).is_ok());
        assert!(cfind_cmd.element(move_dest_tag).is_err());
    }

    // =======================================================================
    // build_cmove_identifier tests
    // =======================================================================

    #[test]
    fn test_build_cmove_identifier_study_level() {
        let ident = build_cmove_identifier("1.2.840.113619.2.55.3.123");

        // QueryRetrieveLevel must be "STUDY"
        let qr_level = ident
            .element(tags::QUERY_RETRIEVE_LEVEL)
            .expect("missing QueryRetrieveLevel");
        assert_eq!(qr_level.to_str().unwrap().trim(), "STUDY");

        // StudyInstanceUID must match
        let study_uid = ident
            .element(tags::STUDY_INSTANCE_UID)
            .expect("missing StudyInstanceUID");
        assert_eq!(
            study_uid.to_str().unwrap().trim_end_matches('\0'),
            "1.2.840.113619.2.55.3.123"
        );
    }

    #[test]
    fn test_build_cmove_identifier_only_has_two_elements() {
        let ident = build_cmove_identifier("1.2.3.4.5");

        // C-MOVE identifier should only have QueryRetrieveLevel + StudyInstanceUID
        let count = ident.iter().count();
        assert_eq!(count, 2, "C-MOVE identifier should have exactly 2 elements");
    }

    #[test]
    fn test_build_cmove_identifier_different_uids() {
        for uid in [
            "1.2.3.4.5",
            "1.2.840.113619.2.55.3.2776660529.492.1624880808.953",
            "2.16.840.1.113669.632.20.121711.10000160881",
        ] {
            let ident = build_cmove_identifier(uid);
            let study_uid = ident
                .element(tags::STUDY_INSTANCE_UID)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches('\0')
                .to_string();
            assert_eq!(study_uid, uid);
        }
    }

    #[test]
    fn test_build_cmove_identifier_serializes() {
        let ident = build_cmove_identifier("1.2.3.4.5");
        let ts = ivr_le!();
        let mut bytes = Vec::new();
        ident
            .write_dataset_with_ts(&mut bytes, &ts)
            .expect("Failed to serialize C-MOVE identifier");
        assert!(!bytes.is_empty());
    }

    // =======================================================================
    // extract_u16_optional tests
    // =======================================================================

    #[test]
    fn test_extract_u16_optional_present() {
        let obj = InMemDicomObject::command_from_element_iter([DataElement::new(
            tags::NUMBER_OF_COMPLETED_SUBOPERATIONS,
            VR::US,
            dicom_value!(U16, [42]),
        )]);
        assert_eq!(
            extract_u16_optional(&obj, tags::NUMBER_OF_COMPLETED_SUBOPERATIONS),
            Some(42)
        );
    }

    #[test]
    fn test_extract_u16_optional_missing() {
        let obj = InMemDicomObject::command_from_element_iter([DataElement::new(
            tags::STATUS,
            VR::US,
            dicom_value!(U16, [0x0000]),
        )]);
        assert!(extract_u16_optional(&obj, tags::NUMBER_OF_COMPLETED_SUBOPERATIONS).is_none());
    }

    #[test]
    fn test_extract_u16_optional_zero() {
        let obj = InMemDicomObject::command_from_element_iter([DataElement::new(
            tags::NUMBER_OF_FAILED_SUBOPERATIONS,
            VR::US,
            dicom_value!(U16, [0]),
        )]);
        assert_eq!(
            extract_u16_optional(&obj, tags::NUMBER_OF_FAILED_SUBOPERATIONS),
            Some(0)
        );
    }

    #[test]
    fn test_extract_u16_optional_max_value() {
        let obj = InMemDicomObject::command_from_element_iter([DataElement::new(
            tags::NUMBER_OF_REMAINING_SUBOPERATIONS,
            VR::US,
            dicom_value!(U16, [0xFFFF]),
        )]);
        assert_eq!(
            extract_u16_optional(&obj, tags::NUMBER_OF_REMAINING_SUBOPERATIONS),
            Some(0xFFFF)
        );
    }

    // =======================================================================
    // describe_cmove_status tests
    // =======================================================================

    #[test]
    fn test_describe_cmove_status_known_codes() {
        assert_eq!(describe_cmove_status(0x0000), "Success");
        assert_eq!(describe_cmove_status(0xFF00), "Pending: sub-operations are continuing");
        assert_eq!(describe_cmove_status(0xB000), "Warning: sub-operations complete, one or more failures or warnings");
        assert_eq!(describe_cmove_status(0xA701), "Refused: out of resources, unable to calculate number of matches");
        assert_eq!(describe_cmove_status(0xA702), "Refused: out of resources, unable to perform sub-operations");
        assert_eq!(describe_cmove_status(0xA801), "Refused: move destination unknown");
        assert_eq!(describe_cmove_status(0xA900), "Identifier does not match SOP Class");
        assert_eq!(describe_cmove_status(0xFE00), "Cancel: sub-operations terminated due to cancel request");
    }

    #[test]
    fn test_describe_cmove_status_cxxx_range() {
        // All codes in the 0xC000-0xCFFF range should map to "Unable to process"
        assert_eq!(describe_cmove_status(0xC000), "Unable to process");
        assert_eq!(describe_cmove_status(0xC001), "Unable to process");
        assert_eq!(describe_cmove_status(0xC500), "Unable to process");
        assert_eq!(describe_cmove_status(0xCFFF), "Unable to process");
    }

    #[test]
    fn test_describe_cmove_status_unknown() {
        assert_eq!(describe_cmove_status(0x1234), "Unknown status");
        assert_eq!(describe_cmove_status(0xDEAD), "Unknown status");
    }

    // =======================================================================
    // execute_cmove integration tests - full C-MOVE against a mock PACS SCP
    // =======================================================================

    /// Test that execute_cmove returns an error when connecting to a
    /// non-existent host.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cmove_connection_refused() {
        let pacs = PacsService {
            ae_title: "NONEXISTENT".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1, // port 1 is almost certainly not running a DICOM SCP
        };

        let result = execute_cmove("BOUNCE", &pacs, "1.2.3.4.5", "BOUNCE").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Failed to establish") || err.contains("timed out"),
            "Unexpected error: {}",
            err
        );
    }

    /// Test a successful C-MOVE against a mock PACS SCP that immediately
    /// returns a Success status (simulating an immediate move completion).
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cmove_success_immediate() {
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
                    .ae_title("MOCK_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                // Accept the Study Root MOVE SOP class
                options =
                    options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.2");

                let mut association = options.establish(std_stream).unwrap();

                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                        .erased();

                // Receive C-MOVE-RQ command + identifier
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

                // Send immediate Success (0x0000) C-MOVE-RSP
                let success_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.2"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8021]), // C-MOVE-RSP
                    ),
                    DataElement::new(
                        tags::MESSAGE_ID_BEING_RESPONDED_TO,
                        VR::US,
                        dicom_value!(U16, [1]),
                    ),
                    DataElement::new(
                        tags::COMMAND_DATA_SET_TYPE,
                        VR::US,
                        dicom_value!(U16, [0x0101]), // No dataset
                    ),
                    DataElement::new(
                        tags::STATUS,
                        VR::US,
                        dicom_value!(U16, [0x0000u16]), // Success
                    ),
                    DataElement::new(
                        tags::NUMBER_OF_COMPLETED_SUBOPERATIONS,
                        VR::US,
                        dicom_value!(U16, [5]),
                    ),
                    DataElement::new(
                        tags::NUMBER_OF_FAILED_SUBOPERATIONS,
                        VR::US,
                        dicom_value!(U16, [0]),
                    ),
                    DataElement::new(
                        tags::NUMBER_OF_WARNING_SUBOPERATIONS,
                        VR::US,
                        dicom_value!(U16, [0]),
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

        let pacs = PacsService {
            ae_title: "MOCK_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let result = execute_cmove("BOUNCE", &pacs, "1.2.3.4.5", "BOUNCE").await;
        assert!(result.is_ok(), "C-MOVE should succeed: {:?}", result.err());

        scp_handle.await.unwrap();
    }

    /// Test a C-MOVE that sends Pending statuses before Success.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cmove_with_pending_then_success() {
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
                    .ae_title("PENDING_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options =
                    options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.2");

                let mut association = options.establish(std_stream).unwrap();

                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                        .erased();

                // Receive C-MOVE-RQ
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

                // Send 3 Pending (0xFF00) status responses
                for remaining in (0..3).rev() {
                    let pending_cmd = InMemDicomObject::command_from_element_iter([
                        DataElement::new(
                            tags::AFFECTED_SOP_CLASS_UID,
                            VR::UI,
                            dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.2"),
                        ),
                        DataElement::new(
                            tags::COMMAND_FIELD,
                            VR::US,
                            dicom_value!(U16, [0x8021]),
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
                            dicom_value!(U16, [0xFF00u16]),
                        ),
                        DataElement::new(
                            tags::NUMBER_OF_REMAINING_SUBOPERATIONS,
                            VR::US,
                            dicom_value!(U16, [remaining as u16]),
                        ),
                        DataElement::new(
                            tags::NUMBER_OF_COMPLETED_SUBOPERATIONS,
                            VR::US,
                            dicom_value!(U16, [(3 - remaining) as u16]),
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
                                value_type: PDataValueType::Command,
                                is_last: true,
                                data: cmd_bytes,
                            }],
                        })
                        .unwrap();
                }

                // Send final Success
                let success_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.2"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8021]),
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
                    DataElement::new(
                        tags::NUMBER_OF_COMPLETED_SUBOPERATIONS,
                        VR::US,
                        dicom_value!(U16, [3]),
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

        let pacs = PacsService {
            ae_title: "PENDING_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let result = execute_cmove("BOUNCE", &pacs, "1.2.840.99999", "BOUNCE").await;
        assert!(result.is_ok(), "C-MOVE should succeed after Pending: {:?}", result.err());

        scp_handle.await.unwrap();
    }

    /// Test that C-MOVE returns an error when the PACS sends a failure status.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cmove_failure_status() {
        use dicom_ul::association::ServerAssociationOptions;
        use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let _scp_handle = tokio::task::spawn_blocking(move || {
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
                    .ae_title("FAIL_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options =
                    options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.2");

                let mut association = options.establish(std_stream).unwrap();

                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                        .erased();

                // Receive C-MOVE-RQ
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

                // Send failure status (0xA701 - unable to calculate number of matches)
                let fail_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.2"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8021]),
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
                        dicom_value!(U16, [0xA701u16]), // Failure
                    ),
                ]);

                let mut cmd_bytes = Vec::new();
                fail_cmd
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

        let pacs = PacsService {
            ae_title: "FAIL_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let result = execute_cmove("BOUNCE", &pacs, "1.2.3.4.5", "BOUNCE").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("0xA701"),
            "Error should mention status code: {}",
            err
        );
        assert!(
            err.contains("Refused: out of resources"),
            "Error should include human-readable description: {}",
            err
        );
    }

    /// Test that C-MOVE failure includes ErrorComment from the PACS when present.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cmove_failure_with_error_comment() {
        use dicom_ul::association::ServerAssociationOptions;
        use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let _scp_handle = tokio::task::spawn_blocking(move || {
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
                    .ae_title("ERRCOMMENT_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options =
                    options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.2");

                let mut association = options.establish(std_stream).unwrap();

                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                        .erased();

                // Receive C-MOVE-RQ
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

                // ErrorComment tag (0000,0902)
                let error_comment_tag = Tag(0x0000, 0x0902);

                // Send failure 0xC000 with ErrorComment
                let fail_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.2"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8021]),
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
                        dicom_value!(U16, [0xC000u16]),
                    ),
                    DataElement::new(
                        error_comment_tag,
                        VR::LO,
                        dicom_value!(Str, "Study not found in archive"),
                    ),
                ]);

                let mut cmd_bytes = Vec::new();
                fail_cmd
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

        let pacs = PacsService {
            ae_title: "ERRCOMMENT_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let result = execute_cmove("BOUNCE", &pacs, "1.2.3.4.5", "BOUNCE").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("0xC000"),
            "Error should mention status code: {}",
            err
        );
        assert!(
            err.contains("Unable to process"),
            "Error should include human-readable description: {}",
            err
        );
        assert!(
            err.contains("Study not found in archive"),
            "Error should include PACS ErrorComment: {}",
            err
        );
    }

    /// Test that C-MOVE handles warning status (0xB000) as success.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cmove_warning_status_succeeds() {
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
                    .ae_title("WARN_PACS")
                    .promiscuous(true);

                for ts in dicom::transfer_syntax::TransferSyntaxRegistry.iter() {
                    if !ts.is_unsupported() {
                        options = options.with_transfer_syntax(ts.uid());
                    }
                }
                options =
                    options.with_abstract_syntax("1.2.840.10008.5.1.4.1.2.2.2");

                let mut association = options.establish(std_stream).unwrap();

                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                        .erased();

                // Receive C-MOVE-RQ
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

                // Send warning status 0xB000 (sub-operations complete, some with warnings)
                let warn_cmd = InMemDicomObject::command_from_element_iter([
                    DataElement::new(
                        tags::AFFECTED_SOP_CLASS_UID,
                        VR::UI,
                        dicom_value!(Str, "1.2.840.10008.5.1.4.1.2.2.2"),
                    ),
                    DataElement::new(
                        tags::COMMAND_FIELD,
                        VR::US,
                        dicom_value!(U16, [0x8021]),
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
                        dicom_value!(U16, [0xB000u16]), // Warning
                    ),
                    DataElement::new(
                        tags::NUMBER_OF_COMPLETED_SUBOPERATIONS,
                        VR::US,
                        dicom_value!(U16, [4]),
                    ),
                    DataElement::new(
                        tags::NUMBER_OF_WARNING_SUBOPERATIONS,
                        VR::US,
                        dicom_value!(U16, [1]),
                    ),
                ]);

                let mut cmd_bytes = Vec::new();
                warn_cmd
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

        let pacs = PacsService {
            ae_title: "WARN_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        // Warning status 0xB000 should be treated as success
        let result = execute_cmove("BOUNCE", &pacs, "1.2.3.4.5", "BOUNCE").await;
        assert!(result.is_ok(), "C-MOVE with warning should succeed: {:?}", result.err());

        scp_handle.await.unwrap();
    }
}
