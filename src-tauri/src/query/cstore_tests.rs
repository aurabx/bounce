#[cfg(test)]
mod tests {
    use crate::query::cstore::{execute_cstore, PROPOSED_TRANSFER_SYNTAXES};
    use crate::query::models::PacsService;
    use dicom::core::{DataElement, VR};
    use dicom::dicom_value;
    use dicom::dictionary_std::tags;
    use dicom::object::{FileDicomObject, FileMetaTableBuilder, InMemDicomObject};

    /// CT Image Storage — used for the fixture instance because every PACS
    /// supports it as an abstract syntax.
    const CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";

    /// Build a minimal in-memory DICOM file object suitable for C-STORE.
    /// Carries the required identifiers (SOP class/instance UIDs, study/series
    /// UIDs, patient ID, modality) but no pixel data.
    fn build_test_instance(sop_instance_uid: &str) -> FileDicomObject<InMemDicomObject> {
        let mut obj = InMemDicomObject::new_empty();
        obj.put(DataElement::new(
            tags::SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, CT_IMAGE_STORAGE),
        ));
        obj.put(DataElement::new(
            tags::SOP_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, sop_instance_uid),
        ));
        obj.put(DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, "1.2.3.4.5.6.7.8.9.STUDY"),
        ));
        obj.put(DataElement::new(
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, "1.2.3.4.5.6.7.8.9.SERIES"),
        ));
        obj.put(DataElement::new(
            tags::PATIENT_ID,
            VR::LO,
            dicom_value!(Str, "TEST-PID"),
        ));
        obj.put(DataElement::new(
            tags::MODALITY,
            VR::CS,
            dicom_value!(Str, "CT"),
        ));

        obj.with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax("1.2.840.10008.1.2.1") // Explicit VR Little Endian
                .media_storage_sop_class_uid(CT_IMAGE_STORAGE)
                .media_storage_sop_instance_uid(sop_instance_uid),
        )
        .expect("FileMetaTableBuilder should produce a valid meta table")
    }

    #[test]
    fn test_proposed_transfer_syntaxes_includes_explicit_vr_le() {
        // Explicit VR Little Endian must be offered first — it is the
        // universal baseline almost every PACS accepts.
        assert_eq!(PROPOSED_TRANSFER_SYNTAXES[0], "1.2.840.10008.1.2.1");
    }

    #[test]
    fn test_proposed_transfer_syntaxes_includes_implicit_vr_le() {
        assert!(PROPOSED_TRANSFER_SYNTAXES.contains(&"1.2.840.10008.1.2"));
    }

    #[test]
    fn test_proposed_transfer_syntaxes_includes_jpeg_baseline() {
        assert!(PROPOSED_TRANSFER_SYNTAXES.contains(&"1.2.840.10008.1.2.4.50"));
    }

    /// An empty instance list must short-circuit without opening an
    /// association — no point making a TCP connection only to release it.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cstore_empty_instances_returns_empty_report() {
        let pacs = PacsService {
            ae_title: "ANY".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1, // would refuse if we actually tried to connect
        };

        let report = execute_cstore("BOUNCE", &pacs, &[]).await.unwrap();
        assert_eq!(report.successes.len(), 0);
        assert_eq!(report.failures.len(), 0);
        assert_eq!(report.total(), 0);
    }

    /// Sanity check: the empty-list short-circuit holds even when the
    /// destination is unreachable. If a future change accidentally tries to
    /// open an association before checking the input, this would hang or
    /// fail with a connection error.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cstore_short_circuits_when_no_instances() {
        let pacs = PacsService {
            ae_title: "UNREACHABLE".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1,
        };

        let result = execute_cstore("BOUNCE", &pacs, &[]).await;
        assert!(result.is_ok());
    }

    /// Connecting to a port that is not running a DICOM SCP must surface a
    /// clear association-failure error rather than hanging or panicking.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cstore_connection_refused() {
        let pacs = PacsService {
            ae_title: "NONEXISTENT".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1,
        };

        let instances = vec![build_test_instance("1.2.3.4.5.6.7.8.9.INST1")];

        let result = execute_cstore("BOUNCE", &pacs, &instances).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("failed to establish") || err.contains("timed out"),
            "unexpected error: {}",
            err
        );
    }

    /// Drive a real C-STORE batch against a mock SCP that accepts CT Image
    /// Storage and acknowledges every instance with status 0x0000. Verifies
    /// that the SCU sends the right commands, parses the responses, and
    /// reports successes back to the caller.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_cstore_success_against_mock_scp() {
        use dicom_ul::association::ServerAssociationOptions;
        use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Mock SCP. Accepts CT Image Storage on any TS the registry supports;
        // for each C-STORE-RQ it receives, it consumes the command + data
        // PDUs then sends back a Success C-STORE-RSP.
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
                options = options.with_abstract_syntax(CT_IMAGE_STORAGE);

                let mut association = options.establish(std_stream).unwrap();

                let command_ts =
                    dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

                // Loop until the SCU releases.
                loop {
                    let pdu = match association.receive() {
                        Ok(p) => p,
                        Err(_) => break,
                    };

                    match pdu {
                        Pdu::PData { data } => {
                            let mut request_pc_id: u8 = 1;
                            let mut request_msg_id: u16 = 1;
                            let mut got_command = false;
                            let mut got_data = false;

                            for dv in &data {
                                match dv.value_type {
                                    PDataValueType::Command if dv.is_last => {
                                        request_pc_id = dv.presentation_context_id;
                                        // Read the command to extract message id.
                                        if let Ok(cmd) = InMemDicomObject::read_dataset_with_ts(
                                            dv.data.as_slice(),
                                            &command_ts,
                                        ) {
                                            if let Ok(elem) = cmd.element(tags::MESSAGE_ID) {
                                                if let Ok(v) = elem.to_int::<u16>() {
                                                    request_msg_id = v;
                                                }
                                            }
                                        }
                                        got_command = true;
                                    }
                                    PDataValueType::Data if dv.is_last => {
                                        got_data = true;
                                    }
                                    _ => {}
                                }
                            }

                            // Wait for the data PDU if it didn't arrive in
                            // the same batch (the SCU sends them as separate
                            // PDUs in our implementation).
                            while got_command && !got_data {
                                let pdu = match association.receive() {
                                    Ok(p) => p,
                                    Err(_) => return,
                                };
                                if let Pdu::PData { data } = pdu {
                                    for dv in &data {
                                        if dv.value_type == PDataValueType::Data && dv.is_last {
                                            got_data = true;
                                        }
                                    }
                                }
                            }

                            if !got_command || !got_data {
                                continue;
                            }

                            // Send Success C-STORE-RSP.
                            let rsp = InMemDicomObject::command_from_element_iter([
                                DataElement::new(
                                    tags::AFFECTED_SOP_CLASS_UID,
                                    VR::UI,
                                    dicom_value!(Str, CT_IMAGE_STORAGE),
                                ),
                                DataElement::new(
                                    tags::COMMAND_FIELD,
                                    VR::US,
                                    dicom_value!(U16, [0x8001u16]), // C-STORE-RSP
                                ),
                                DataElement::new(
                                    tags::MESSAGE_ID_BEING_RESPONDED_TO,
                                    VR::US,
                                    dicom_value!(U16, [request_msg_id]),
                                ),
                                DataElement::new(
                                    tags::COMMAND_DATA_SET_TYPE,
                                    VR::US,
                                    dicom_value!(U16, [0x0101u16]), // No dataset
                                ),
                                DataElement::new(
                                    tags::STATUS,
                                    VR::US,
                                    dicom_value!(U16, [0x0000u16]), // Success
                                ),
                            ]);

                            let mut bytes = Vec::new();
                            rsp.write_dataset_with_ts(&mut bytes, &command_ts).unwrap();

                            association
                                .send(&Pdu::PData {
                                    data: vec![PDataValue {
                                        presentation_context_id: request_pc_id,
                                        value_type: PDataValueType::Command,
                                        is_last: true,
                                        data: bytes,
                                    }],
                                })
                                .unwrap();
                        }
                        Pdu::ReleaseRQ => {
                            let _ = association.send(&Pdu::ReleaseRP);
                            break;
                        }
                        _ => {}
                    }
                }
            });
        });

        let pacs = PacsService {
            ae_title: "MOCK_PACS".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let instances = vec![
            build_test_instance("1.2.3.4.5.6.7.8.9.INST1"),
            build_test_instance("1.2.3.4.5.6.7.8.9.INST2"),
        ];

        let report = execute_cstore("BOUNCE", &pacs, &instances)
            .await
            .expect("C-STORE batch should succeed against the mock SCP");

        assert_eq!(report.successes.len(), 2);
        assert_eq!(report.failures.len(), 0);
        assert_eq!(report.total(), 2);

        // Each successful instance should report status 0x0000.
        for r in &report.successes {
            assert_eq!(r.status, 0x0000);
            assert!(r.is_success());
        }

        scp_handle.await.unwrap();
    }
}
