//! C-MOVE SCU (Service Class User) implementation.
//!
//! Establishes an outbound DICOM association to a PACS and executes a
//! Study Root C-MOVE request, instructing the PACS to send the study
//! to Bounce's C-STORE SCP.
//!
//! After the C-MOVE completes, the PACS will have sent the study images
//! to Bounce's DICOM server, which handles receiving and uploading them
//! through the normal upload workflow automatically.

use crate::query::cfind::{extract_status, extract_string_optional};
use crate::query::models::PacsService;
use crate::{log_error, log_info};
use dicom::core::{DataElement, Tag, VR};
use dicom::core::header::Header;
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::object::{InMemDicomObject, StandardDataDictionary};
use dicom::encoding::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::association::ClientAssociationOptions;
use dicom_ul::pdu::{PDataValueType, PresentationContextResultReason, Pdu};
use std::time::Duration;

/// Study Root Query/Retrieve Information Model - MOVE
const STUDY_ROOT_MOVE_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.2.2.2";

/// Maximum time to wait for the entire C-MOVE operation.
///
/// C-MOVE can take significantly longer than C-FIND because the PACS
/// needs to send potentially large studies to Bounce's SCP. We use a
/// generous timeout.
const CMOVE_TIMEOUT: Duration = Duration::from_secs(300);

/// Execute a C-MOVE request against a remote PACS.
///
/// This tells the PACS to send the study identified by `study_instance_uid`
/// to Bounce's C-STORE SCP (identified by `move_destination_ae`).
///
/// # Arguments
///
/// * `calling_ae` - The AE title of this Bounce gateway (our SCU identity).
/// * `pacs` - Connection details for the target PACS SCP.
/// * `study_instance_uid` - The StudyInstanceUID of the study to retrieve.
/// * `move_destination_ae` - The AE title of Bounce's C-STORE SCP that the
///   PACS should send files to. This is typically the same as `calling_ae`.
///
/// # Returns
///
/// `Ok(())` if the C-MOVE completed successfully, or an error string.
pub async fn execute_cmove(
    calling_ae: &str,
    pacs: &PacsService,
    study_instance_uid: &str,
    move_destination_ae: &str,
) -> Result<(), String> {
    let result = tokio::time::timeout(CMOVE_TIMEOUT, async {
        execute_cmove_inner(calling_ae, pacs, study_instance_uid, move_destination_ae).await
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(format!(
            "C-MOVE timed out after {}s retrieving study {} from {}@{}:{}",
            CMOVE_TIMEOUT.as_secs(),
            study_instance_uid,
            pacs.ae_title,
            pacs.host,
            pacs.port,
        )),
    }
}

/// Inner implementation without timeout wrapper.
async fn execute_cmove_inner(
    calling_ae: &str,
    pacs: &PacsService,
    study_instance_uid: &str,
    move_destination_ae: &str,
) -> Result<(), String> {
    let addr = format!("{}:{}", pacs.host, pacs.port);

    log_info!(
        "C-MOVE: establishing association to {} (AE: {}) from {} for study {}, move_destination={}",
        addr,
        pacs.ae_title,
        calling_ae,
        study_instance_uid,
        move_destination_ae,
    );

    // Establish association as SCU with Study Root MOVE SOP class.
    let mut association = ClientAssociationOptions::new()
        .calling_ae_title(calling_ae)
        .called_ae_title(&pacs.ae_title)
        .with_abstract_syntax(STUDY_ROOT_MOVE_SOP_CLASS)
        .establish_async(&addr)
        .await
        .map_err(|e| format!("Failed to establish DICOM association with {}: {}", addr, e))?;

    log_info!("C-MOVE: association established with {}", pacs.ae_title);

    // Find the accepted presentation context for Study Root MOVE.
    let pc = association
        .presentation_contexts()
        .first()
        .ok_or_else(|| {
            format!(
                "PACS {} did not accept Study Root MOVE presentation context",
                pacs.ae_title,
            )
        })?;

    if pc.reason != PresentationContextResultReason::Acceptance {
        return Err(format!(
            "PACS {} rejected Study Root MOVE presentation context: {:?}",
            pacs.ae_title, pc.reason,
        ));
    }

    let pc_id = pc.id;

    // Resolve the negotiated transfer syntax for the identifier dataset.
    let negotiated_ts = TransferSyntaxRegistry
        .get(&pc.transfer_syntax)
        .ok_or_else(|| {
            format!(
                "Unsupported transfer syntax negotiated with {}: {}",
                pacs.ae_title, pc.transfer_syntax,
            )
        })?;

    log_info!(
        "C-MOVE: negotiated transfer syntax: {} ({})",
        negotiated_ts.name(),
        pc.transfer_syntax,
    );

    // DICOM commands are always encoded in Implicit VR Little Endian.
    let command_ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    // Build the C-MOVE identifier: QueryRetrieveLevel=STUDY + StudyInstanceUID
    let identifier = build_cmove_identifier(study_instance_uid);

    // Serialize the identifier with the negotiated transfer syntax.
    let mut identifier_bytes = Vec::new();
    identifier
        .write_dataset_with_ts(&mut identifier_bytes, negotiated_ts)
        .map_err(|e| format!("Failed to serialize C-MOVE identifier: {}", e))?;

    log_info!(
        "C-MOVE: identifier: QueryRetrieveLevel=STUDY, StudyInstanceUID={} ({} bytes)",
        study_instance_uid,
        identifier_bytes.len(),
    );

    // Build the C-MOVE-RQ command object.
    let command = build_cmove_command(1, move_destination_ae);

    let mut command_bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut command_bytes, &command_ts)
        .map_err(|e| format!("Failed to serialize C-MOVE command: {}", e))?;

    // Send command PDU.
    let command_pdu = Pdu::PData {
        data: vec![dicom_ul::pdu::PDataValue {
            presentation_context_id: pc_id,
            value_type: PDataValueType::Command,
            is_last: true,
            data: command_bytes,
        }],
    };

    association
        .send(&command_pdu)
        .await
        .map_err(|e| format!("Failed to send C-MOVE-RQ command: {}", e))?;

    // Send identifier (data) PDU.
    let data_pdu = Pdu::PData {
        data: vec![dicom_ul::pdu::PDataValue {
            presentation_context_id: pc_id,
            value_type: PDataValueType::Data,
            is_last: true,
            data: identifier_bytes,
        }],
    };

    association
        .send(&data_pdu)
        .await
        .map_err(|e| format!("Failed to send C-MOVE identifier: {}", e))?;

    log_info!("C-MOVE: request sent, awaiting status responses");

    // Receive C-MOVE-RSP status messages.
    //
    // The PACS sends Pending (0xFF00) status responses as it processes
    // sub-operations (sending files to our SCP), then a final Success
    // (0x0000) or failure status when done.
    loop {
        let pdu = association
            .receive()
            .await
            .map_err(|e| format!("Failed to receive C-MOVE response: {}", e))?;

        match pdu {
            Pdu::PData { ref data } => {
                for data_value in data {
                    match data_value.value_type {
                        PDataValueType::Command if data_value.is_last => {
                            let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                                data_value.data.as_slice(),
                                &command_ts,
                            )
                            .map_err(|e| {
                                format!("Failed to parse C-MOVE-RSP command: {}", e)
                            })?;

                            let status = extract_status(&cmd_obj)?;

                            // Extract sub-operation counts if available
                            let remaining = extract_u16_optional(&cmd_obj, tags::NUMBER_OF_REMAINING_SUBOPERATIONS);
                            let completed = extract_u16_optional(&cmd_obj, tags::NUMBER_OF_COMPLETED_SUBOPERATIONS);
                            let failed = extract_u16_optional(&cmd_obj, tags::NUMBER_OF_FAILED_SUBOPERATIONS);
                            let warning = extract_u16_optional(&cmd_obj, tags::NUMBER_OF_WARNING_SUBOPERATIONS);

                            log_info!(
                                "C-MOVE: status=0x{:04X} remaining={:?} completed={:?} failed={:?} warning={:?}",
                                status, remaining, completed, failed, warning,
                            );

                            match status {
                                0x0000 => {
                                    // Success - all sub-operations completed.
                                    log_info!(
                                        "C-MOVE: completed successfully for study {}",
                                        study_instance_uid,
                                    );

                                    if let Err(e) = association.release().await {
                                        log_error!("C-MOVE: failed to release association: {}", e);
                                    }

                                    return Ok(());
                                }
                                0xFF00 => {
                                    // Pending - sub-operations still in progress.
                                    // Continue waiting for more status responses.
                                }
                                0xB000 => {
                                    // Warning - one or more sub-operations had
                                    // warnings but the move itself completed.
                                    log_info!(
                                        "C-MOVE: completed with warnings for study {}",
                                        study_instance_uid,
                                    );

                                    if let Err(e) = association.release().await {
                                        log_error!("C-MOVE: failed to release association: {}", e);
                                    }

                                    return Ok(());
                                }
                                _ => {
                                    // Error or cancel status.
                                    let status_desc = describe_cmove_status(status);

                                    // ErrorComment (0000,0902) - the PACS may include
                                    // a diagnostic message explaining the failure.
                                    let error_comment_tag = Tag(0x0000, 0x0902);
                                    let error_comment = extract_string_optional(
                                        &cmd_obj, error_comment_tag,
                                    );

                                    // Log ALL elements from the response for diagnostics.
                                    log_error!(
                                        "C-MOVE: failed with status 0x{:04X} ({}) for study {} from {}",
                                        status, status_desc, study_instance_uid, pacs.ae_title,
                                    );
                                    if let Some(ref comment) = error_comment {
                                        log_error!("C-MOVE: PACS ErrorComment: {}", comment);
                                    }
                                    log_info!("C-MOVE: full response command elements:");
                                    log_cmove_response_details(&cmd_obj);

                                    // Build an informative error message for Aura.
                                    let msg = match error_comment {
                                        Some(comment) => format!(
                                            "C-MOVE failed with status 0x{:04X} ({}) for study {}: {}",
                                            status, status_desc, study_instance_uid, comment,
                                        ),
                                        None => format!(
                                            "C-MOVE failed with status 0x{:04X} ({}) for study {}",
                                            status, status_desc, study_instance_uid,
                                        ),
                                    };

                                    if let Err(e) = association.release().await {
                                        log_error!("C-MOVE: failed to release association: {}", e);
                                    }

                                    return Err(msg);
                                }
                            }
                        }
                        PDataValueType::Data => {
                            // C-MOVE-RSP may include a dataset with failed
                            // SOP instance UIDs on error. We log but don't
                            // parse further.
                            log_info!(
                                "C-MOVE: received response data ({} bytes)",
                                data_value.data.len(),
                            );
                        }
                        _ => {
                            // Partial command fragment - unusual but handle gracefully.
                        }
                    }
                }
            }
            Pdu::ReleaseRQ => {
                log_info!("C-MOVE: PACS sent ReleaseRQ unexpectedly");
                let _ = association.send(&Pdu::ReleaseRP).await;
                return Err("PACS released association before C-MOVE completed".to_string());
            }
            Pdu::AbortRQ { source } => {
                return Err(format!("C-MOVE: PACS aborted association: {:?}", source));
            }
            _ => {
                log_info!("C-MOVE: ignoring unexpected PDU");
            }
        }
    }
}

/// Build the C-MOVE-RQ command object.
///
/// The key differences from C-FIND-RQ are:
/// - `AffectedSOPClassUID` uses the Study Root MOVE SOP class
/// - `CommandField` is 0x0021 (C-MOVE-RQ) instead of 0x0020 (C-FIND-RQ)
/// - `MoveDestination` (0000,0600) specifies where the PACS should send files
pub(crate) fn build_cmove_command(
    message_id: u16,
    move_destination: &str,
) -> InMemDicomObject<StandardDataDictionary> {
    // MoveDestination tag (0000,0600)
    let move_destination_tag = Tag(0x0000, 0x0600);

    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, STUDY_ROOT_MOVE_SOP_CLASS),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x0021]), // C-MOVE-RQ
        ),
        DataElement::new(
            tags::MESSAGE_ID,
            VR::US,
            dicom_value!(U16, [message_id]),
        ),
        DataElement::new(
            tags::PRIORITY,
            VR::US,
            dicom_value!(U16, [0x0000]), // Medium priority
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0001]), // Dataset present (identifier follows)
        ),
        DataElement::new(
            move_destination_tag,
            VR::AE,
            dicom_value!(Str, move_destination),
        ),
    ])
}

/// Build the C-MOVE identifier dataset.
///
/// For a study-level retrieve, we only need:
/// - `QueryRetrieveLevel` = `"STUDY"`
/// - `StudyInstanceUID` = the UID of the study to retrieve
pub(crate) fn build_cmove_identifier(study_instance_uid: &str) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::QUERY_RETRIEVE_LEVEL,
            VR::CS,
            dicom_value!(Str, "STUDY"),
        ),
        DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, study_instance_uid),
        ),
    ])
}

/// Extract a u16 value from a command object, returning `None` if missing.
pub(crate) fn extract_u16_optional(obj: &InMemDicomObject<StandardDataDictionary>, tag: Tag) -> Option<u16> {
    match obj.element(tag) {
        Ok(elem) => elem.to_int::<u16>().ok(),
        Err(_) => None,
    }
}

/// Return a human-readable description for a C-MOVE response status code.
///
/// Based on DICOM PS3.4 Annex C (Query/Retrieve Service Class).
pub(crate) fn describe_cmove_status(status: u16) -> &'static str {
    match status {
        0x0000 => "Success",
        0xFF00 => "Pending: sub-operations are continuing",
        0xB000 => "Warning: sub-operations complete, one or more failures or warnings",
        0xA701 => "Refused: out of resources, unable to calculate number of matches",
        0xA702 => "Refused: out of resources, unable to perform sub-operations",
        0xA801 => "Refused: move destination unknown",
        0xA900 => "Identifier does not match SOP Class",
        0xFE00 => "Cancel: sub-operations terminated due to cancel request",
        s if (0xC000..=0xCFFF).contains(&s) => "Unable to process",
        _ => "Unknown status",
    }
}

/// Log all elements of a C-MOVE-RSP command object for diagnostic purposes.
fn log_cmove_response_details(cmd_obj: &InMemDicomObject<StandardDataDictionary>) {
    for elem in cmd_obj.into_iter() {
        let tag = elem.tag();
        let value = match elem.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => format!("{:?}", elem.value()),
        };
        log_info!("C-MOVE-RSP element: ({:04X},{:04X}) = {}", tag.0, tag.1, value);
    }
}
