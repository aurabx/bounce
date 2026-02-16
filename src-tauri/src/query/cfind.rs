//! C-FIND SCU (Service Class User) implementation.
//!
//! Establishes an outbound DICOM association to a PACS and executes a
//! Study Root C-FIND query, collecting results into [`CfindResult`] values.

use crate::query::models::{CfindResult, PacsService, QueryFilters};
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

/// Study Root Query/Retrieve Information Model - FIND
const STUDY_ROOT_FIND_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.2.2.1";

/// Maximum time to wait for the entire C-FIND operation (association + query + results).
const CFIND_TIMEOUT: Duration = Duration::from_secs(30);

/// Execute a C-FIND query against a remote PACS.
///
/// # Arguments
///
/// * `calling_ae` - The AE title of this Bounce gateway (our SCU identity).
/// * `pacs` - Connection details for the target PACS SCP.
/// * `filters` - Query match keys.
///
/// # Returns
///
/// A vector of [`CfindResult`] study-level records, or an error string.
pub async fn execute_cfind(
    calling_ae: &str,
    pacs: &PacsService,
    filters: &QueryFilters,
) -> Result<Vec<CfindResult>, String> {
    // Run the entire operation under a timeout
    let result = tokio::time::timeout(CFIND_TIMEOUT, async {
        execute_cfind_inner(calling_ae, pacs, filters).await
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(format!(
            "C-FIND timed out after {}s querying {}@{}:{}",
            CFIND_TIMEOUT.as_secs(),
            pacs.ae_title,
            pacs.host,
            pacs.port,
        )),
    }
}

/// Inner implementation without timeout wrapper.
async fn execute_cfind_inner(
    calling_ae: &str,
    pacs: &PacsService,
    filters: &QueryFilters,
) -> Result<Vec<CfindResult>, String> {
    let addr = format!("{}:{}", pacs.host, pacs.port);

    log_info!(
        "C-FIND: establishing association to {} (AE: {}) from {}",
        addr,
        pacs.ae_title,
        calling_ae,
    );

    // Establish association as SCU.
    // Transfer syntaxes (Implicit VR LE, Explicit VR LE) are proposed
    // automatically by ClientAssociationOptions for the abstract syntax.
    let mut association = ClientAssociationOptions::new()
        .calling_ae_title(calling_ae)
        .called_ae_title(&pacs.ae_title)
        .with_abstract_syntax(STUDY_ROOT_FIND_SOP_CLASS)
        .establish_async(&addr)
        .await
        .map_err(|e| format!("Failed to establish DICOM association with {}: {}", addr, e))?;

    log_info!(
        "C-FIND: association established with {}",
        pacs.ae_title,
    );

    // Find the accepted presentation context. Since we only proposed one
    // abstract syntax (Study Root FIND), the first accepted context is it.
    let pc = association
        .presentation_contexts()
        .first()
        .ok_or_else(|| {
            format!(
                "PACS {} did not accept Study Root FIND presentation context",
                pacs.ae_title,
            )
        })?;

    // Verify the presentation context was actually accepted
    if pc.reason != PresentationContextResultReason::Acceptance {
        return Err(format!(
            "PACS {} rejected Study Root FIND presentation context: {:?}",
            pacs.ae_title, pc.reason,
        ));
    }

    let pc_id = pc.id;

    // Resolve the negotiated transfer syntax for serializing/deserializing
    // the identifier and response datasets.
    let negotiated_ts = TransferSyntaxRegistry
        .get(&pc.transfer_syntax)
        .ok_or_else(|| {
            format!(
                "Unsupported transfer syntax negotiated with {}: {}",
                pacs.ae_title, pc.transfer_syntax,
            )
        })?;

    log_info!(
        "C-FIND: negotiated transfer syntax: {} ({})",
        negotiated_ts.name(),
        pc.transfer_syntax,
    );

    // DICOM commands are always encoded in Implicit VR Little Endian
    // (PS3.7, Section 6.3.1), regardless of the negotiated transfer syntax.
    let command_ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    // Build the C-FIND identifier (the data object that carries match/return keys)
    let identifier = build_cfind_identifier(filters);

    // Serialize the identifier with the negotiated transfer syntax
    let mut identifier_bytes = Vec::new();
    identifier
        .write_dataset_with_ts(&mut identifier_bytes, negotiated_ts)
        .map_err(|e| format!("Failed to serialize C-FIND identifier: {}", e))?;

    // Debug: dump the identifier elements and raw bytes
    for elem in identifier.iter() {
        log_info!(
            "C-FIND identifier tag {:04X},{:04X} VR={:?} value={:?}",
            elem.tag().group(),
            elem.tag().element(),
            elem.vr(),
            elem.to_str().unwrap_or_default(),
        );
    }
    log_info!("C-FIND identifier bytes ({} bytes): {:02X?}", identifier_bytes.len(), &identifier_bytes);

    // Build the C-FIND-RQ command object
    let command = build_cfind_command(1); // message ID = 1

    let mut command_bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut command_bytes, &command_ts)
        .map_err(|e| format!("Failed to serialize C-FIND command: {}", e))?;

    // Send command PDU
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
        .map_err(|e| format!("Failed to send C-FIND-RQ command: {}", e))?;

    // Send identifier (data) PDU
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
        .map_err(|e| format!("Failed to send C-FIND identifier: {}", e))?;

    log_info!("C-FIND: request sent, awaiting responses");

    // Receive responses
    let mut results: Vec<CfindResult> = Vec::new();
    let mut response_data_buffer: Vec<u8> = Vec::new();

    loop {
        let pdu = association
            .receive()
            .await
            .map_err(|e| format!("Failed to receive C-FIND response: {}", e))?;

        match pdu {
            Pdu::PData { ref data } => {
                log_info!("C-FIND: received PData with {} fragments", data.len());
                for data_value in data {
                    log_info!(
                        "C-FIND: fragment type={:?} is_last={} len={}",
                        data_value.value_type,
                        data_value.is_last,
                        data_value.data.len(),
                    );
                    match data_value.value_type {
                        PDataValueType::Command if data_value.is_last => {
                            // Parse the command response (always Implicit VR LE)
                            let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                                data_value.data.as_slice(),
                                &command_ts,
                            )
                            .map_err(|e| {
                                format!("Failed to parse C-FIND-RSP command: {}", e)
                            })?;

                            let status = extract_status(&cmd_obj)?;
                            log_info!("C-FIND: command status=0x{:04X} buffer_len={}", status, response_data_buffer.len());

                            match status {
                                0x0000 => {
                                    // Flush any remaining buffered data from a
                                    // preceding Pending response whose data
                                    // arrived in a separate PData PDU.
                                    if !response_data_buffer.is_empty() {
                                        match parse_cfind_result(&response_data_buffer, negotiated_ts) {
                                            Ok(result) => results.push(result),
                                            Err(e) => {
                                                log_error!(
                                                    "C-FIND: failed to parse final buffered result: {}",
                                                    e,
                                                );
                                            }
                                        }
                                        response_data_buffer.clear();
                                    }

                                    log_info!(
                                        "C-FIND: completed with {} results",
                                        results.len()
                                    );

                                    // Release the association gracefully
                                    if let Err(e) = association.release().await {
                                        log_error!("C-FIND: failed to release association: {}", e);
                                    }

                                    return Ok(results);
                                }
                                0xFF00 | 0xFF01 => {
                                    // Pending - a matching result should follow in data fragments.
                                    // The data may have already arrived in preceding Data fragments
                                    // within this same PData PDU sequence, or it will come in the
                                    // next PData PDU(s).
                                    //
                                    // If we already have buffered data, parse it now.
                                    if !response_data_buffer.is_empty() {
                                        match parse_cfind_result(&response_data_buffer, negotiated_ts) {
                                            Ok(result) => results.push(result),
                                            Err(e) => {
                                                log_error!(
                                                    "C-FIND: failed to parse result: {}",
                                                    e,
                                                );
                                            }
                                        }
                                        response_data_buffer.clear();
                                    }

                                    if status == 0xFF01 {
                                        log_info!("C-FIND: pending response with warnings");
                                    }
                                }
                                _ => {
                                    // Error status
                                    let msg = format!(
                                        "C-FIND failed with status 0x{:04X}",
                                        status,
                                    );
                                    log_error!("{}", msg);

                                    if let Err(e) = association.release().await {
                                        log_error!("C-FIND: failed to release association: {}", e);
                                    }

                                    return Err(msg);
                                }
                            }
                        }
                        PDataValueType::Data => {
                            // Accumulate data fragments
                            response_data_buffer.extend_from_slice(&data_value.data);
                        }
                        _ => {
                            // Command fragment that is not last -- accumulate if needed
                            // (unusual for C-FIND responses but handle gracefully)
                        }
                    }
                }
            }
            Pdu::ReleaseRQ => {
                log_info!("C-FIND: PACS sent ReleaseRQ unexpectedly");
                let _ = association.send(&Pdu::ReleaseRP).await;
                break;
            }
            Pdu::AbortRQ { source } => {
                return Err(format!("C-FIND: PACS aborted association: {:?}", source));
            }
            _ => {
                log_info!("C-FIND: ignoring unexpected PDU");
            }
        }
    }

    Ok(results)
}

/// Build the C-FIND-RQ command object.
pub(crate) fn build_cfind_command(message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, STUDY_ROOT_FIND_SOP_CLASS),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x0020]), // C-FIND-RQ
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
            dicom_value!(U16, [0x0001]), // Dataset present (identifier follows); 0x0101 = no dataset
        ),
    ])
}

/// Build the C-FIND identifier dataset with match/return keys.
///
/// Populated filter fields act as match keys; empty elements act as
/// return keys requesting the PACS to populate them.
pub(crate) fn build_cfind_identifier(filters: &QueryFilters) -> InMemDicomObject<StandardDataDictionary> {
    let elements: Vec<DataElement<InMemDicomObject<StandardDataDictionary>>> = vec![
        // Required: QueryRetrieveLevel
        DataElement::new(
            tags::QUERY_RETRIEVE_LEVEL,
            VR::CS,
            dicom_value!(Str, "STUDY"),
        ),
        // StudyDate (0008,0020)
        DataElement::new(
            tags::STUDY_DATE,
            VR::DA,
            match &filters.study_date {
                Some(d) => dicom_value!(Str, d.as_str()),
                None => dicom_value!(),
            },
        ),
        // StudyTime (0008,0030) - return key only
        DataElement::new(tags::STUDY_TIME, VR::TM, dicom_value!()),
        // AccessionNumber (0008,0050)
        DataElement::new(
            tags::ACCESSION_NUMBER,
            VR::SH,
            match &filters.accession_number {
                Some(a) => dicom_value!(Str, a.as_str()),
                None => dicom_value!(),
            },
        ),
        // ModalitiesInStudy (0008,0061)
        DataElement::new(
            tags::MODALITIES_IN_STUDY,
            VR::CS,
            match &filters.modality {
                Some(m) => dicom_value!(Str, m.as_str()),
                None => dicom_value!(),
            },
        ),
        // StudyDescription (0008,1030) - return key
        DataElement::new(tags::STUDY_DESCRIPTION, VR::LO, dicom_value!()),
        // PatientName (0010,0010)
        DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            match &filters.patient_name {
                Some(n) => dicom_value!(Str, n.as_str()),
                None => dicom_value!(),
            },
        ),
        // PatientID (0010,0020)
        DataElement::new(
            tags::PATIENT_ID,
            VR::LO,
            match &filters.patient_id {
                Some(id) => dicom_value!(Str, id.as_str()),
                None => dicom_value!(),
            },
        ),
        // PatientBirthDate (0010,0030) - return key
        DataElement::new(Tag(0x0010, 0x0030), VR::DA, dicom_value!()),
        // PatientSex (0010,0040) - return key
        DataElement::new(Tag(0x0010, 0x0040), VR::CS, dicom_value!()),
        // StudyInstanceUID (0020,000D) - return key (always needed)
        DataElement::new(tags::STUDY_INSTANCE_UID, VR::UI, dicom_value!()),
        // StudyID (0020,0010) - return key
        DataElement::new(Tag(0x0020, 0x0010), VR::SH, dicom_value!()),
        // NumberOfStudyRelatedSeries (0020,1206) - return key
        DataElement::new(Tag(0x0020, 0x1206), VR::IS, dicom_value!()),
        // NumberOfStudyRelatedInstances (0020,1208) - return key
        DataElement::new(Tag(0x0020, 0x1208), VR::IS, dicom_value!()),
    ];

    InMemDicomObject::from_element_iter(elements)
}

/// Parse a C-FIND response data object into a [`CfindResult`].
///
/// The `ts` parameter must be the transfer syntax negotiated during
/// association establishment so that response datasets are decoded
/// correctly.
pub(crate) fn parse_cfind_result(data: &[u8], ts: &dicom::encoding::TransferSyntax) -> Result<CfindResult, String> {
    let obj = InMemDicomObject::read_dataset_with_ts(data, ts)
        .map_err(|e| format!("Failed to parse C-FIND result dataset: {}", e))?;

    // StudyInstanceUID is required
    let study_instance_uid = extract_string_required(&obj, tags::STUDY_INSTANCE_UID)
        .ok_or("C-FIND result missing StudyInstanceUID")?;

    Ok(CfindResult {
        patient_name: extract_string_optional(&obj, tags::PATIENT_NAME),
        patient_id: extract_string_optional(&obj, tags::PATIENT_ID),
        study_date: extract_string_optional(&obj, tags::STUDY_DATE),
        study_time: extract_string_optional(&obj, tags::STUDY_TIME),
        study_description: extract_string_optional(&obj, tags::STUDY_DESCRIPTION),
        accession_number: extract_string_optional(&obj, tags::ACCESSION_NUMBER),
        study_instance_uid,
        modalities_in_study: extract_string_optional(&obj, tags::MODALITIES_IN_STUDY),
        number_of_series: extract_integer_optional(&obj, Tag(0x0020, 0x1206)),
        number_of_instances: extract_integer_optional(&obj, Tag(0x0020, 0x1208)),
    })
}

/// Extract the Status (0000,0900) from a C-FIND-RSP command object.
pub(crate) fn extract_status(obj: &InMemDicomObject<StandardDataDictionary>) -> Result<u16, String> {
    obj.element(tags::STATUS)
        .map_err(|_| "C-FIND-RSP missing Status tag".to_string())?
        .to_int::<u16>()
        .map_err(|e| format!("Failed to read Status value: {}", e))
}

/// Extract a string tag, returning `None` if missing or empty.
pub(crate) fn extract_string_optional(obj: &InMemDicomObject<StandardDataDictionary>, tag: Tag) -> Option<String> {
    match obj.element(tag) {
        Ok(elem) => match elem.to_str() {
            Ok(s) => {
                let s = s.trim_end_matches('\0').trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            Err(_) => None,
        },
        Err(_) => None,
    }
}

/// Extract a required string tag, returning `None` if missing or empty.
pub(crate) fn extract_string_required(obj: &InMemDicomObject<StandardDataDictionary>, tag: Tag) -> Option<String> {
    extract_string_optional(obj, tag)
}

/// Extract an integer from an IS (Integer String) element, returning `None` on failure.
pub(crate) fn extract_integer_optional(obj: &InMemDicomObject<StandardDataDictionary>, tag: Tag) -> Option<u32> {
    match obj.element(tag) {
        Ok(elem) => match elem.to_str() {
            Ok(s) => s.trim().parse::<u32>().ok(),
            Err(_) => None,
        },
        Err(_) => None,
    }
}
