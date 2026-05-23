//! C-FIND SCU (Service Class User) implementation.
//!
//! Establishes an outbound DICOM association to a PACS and executes a
//! Study Root C-FIND query, collecting results into [`CfindResult`] values.

use crate::dimse;
use crate::query::models::{CfindResult, PacsService, QueryFilters};
use crate::{log_error, log_info};
use dicom::core::header::Header;
use dicom::core::{DataElement, Tag, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::encoding::TransferSyntaxIndex;
use dicom::object::{InMemDicomObject, StandardDataDictionary};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::association::ClientAssociationOptions;
use dicom_ul::pdu::{PDataValueType, Pdu, PresentationContextResultReason};
use std::time::Duration;

/// Study Root Query/Retrieve Information Model - FIND
const STUDY_ROOT_FIND_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.2.2.1";

/// Maximum time to wait for the entire C-FIND operation (association + query + results).
const CFIND_TIMEOUT: Duration = Duration::from_secs(30);

/// Message ID used for outbound C-FIND requests on a fresh association.
const CFIND_MESSAGE_ID: u16 = 1;

/// Execute a C-FIND query against a remote PACS.
///
/// # Arguments
///
/// * `calling_ae` - The AE title of this Bounce gateway (our SCU identity).
/// * `pacs` - Connection details for the target PACS SCP.
/// * `query_level` - The DICOM Query/Retrieve level: `"PATIENT"`, `"STUDY"`, or `"SERIES"`.
/// * `filters` - Query match keys.
///
/// # Returns
///
/// A vector of [`CfindResult`] records, or an error string.
pub async fn execute_cfind(
    calling_ae: &str,
    pacs: &PacsService,
    query_level: &str,
    filters: &QueryFilters,
) -> Result<Vec<CfindResult>, String> {
    // Run the entire operation under a timeout
    let level = query_level.to_string();
    let result = tokio::time::timeout(CFIND_TIMEOUT, async {
        execute_cfind_inner(calling_ae, pacs, &level, filters).await
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
    query_level: &str,
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

    log_info!("C-FIND: association established with {}", pacs.ae_title,);

    // Find the accepted presentation context. Since we only proposed one
    // abstract syntax (Study Root FIND), the first accepted context is it.
    let pc = association.presentation_contexts().first().ok_or_else(|| {
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
    let identifier = build_cfind_identifier(query_level, filters);

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
    log_info!(
        "C-FIND identifier bytes ({} bytes): {:02X?}",
        identifier_bytes.len(),
        &identifier_bytes
    );

    // Build the C-FIND-RQ command object
    let command = build_cfind_command(CFIND_MESSAGE_ID);

    dimse::log_scu_request(
        &pacs.ae_title,
        &addr,
        "C-FIND-RQ",
        CFIND_MESSAGE_ID,
        &[
            ("query_level", query_level.to_string()),
            (
                "patient_id",
                dimse::format_optional_str(filters.patient_id.as_deref()),
            ),
            (
                "patient_name",
                dimse::format_optional_str(filters.patient_name.as_deref()),
            ),
            (
                "accession_number",
                dimse::format_optional_str(filters.accession_number.as_deref()),
            ),
            (
                "study_date",
                dimse::format_optional_str(filters.study_date.as_deref()),
            ),
            (
                "study_instance_uid",
                dimse::format_optional_str(filters.study_instance_uid.as_deref()),
            ),
            (
                "modality",
                dimse::format_optional_str(filters.modality.as_deref()),
            ),
        ],
    );

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

    log_info!(
        "C-FIND: request sent for msg_id={}, awaiting responses",
        CFIND_MESSAGE_ID,
    );

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
                            .map_err(|e| format!("Failed to parse C-FIND-RSP command: {}", e))?;

                            let status = extract_status(&cmd_obj)?;
                            let response_message_id =
                                extract_u16_optional(&cmd_obj, tags::MESSAGE_ID_BEING_RESPONDED_TO);
                            dimse::log_scu_response(
                                &pacs.ae_title,
                                &addr,
                                "C-FIND-RSP",
                                response_message_id,
                                status,
                                &[("buffer_len", response_data_buffer.len().to_string())],
                            );

                            match status {
                                0x0000 => {
                                    // Flush any remaining buffered data from a
                                    // preceding Pending response whose data
                                    // arrived in a separate PData PDU.
                                    if !response_data_buffer.is_empty() {
                                        match parse_cfind_result(
                                            &response_data_buffer,
                                            negotiated_ts,
                                            query_level,
                                        ) {
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

                                    log_info!("C-FIND: completed with {} results", results.len());

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
                                        match parse_cfind_result(
                                            &response_data_buffer,
                                            negotiated_ts,
                                            query_level,
                                        ) {
                                            Ok(result) => results.push(result),
                                            Err(e) => {
                                                log_error!("C-FIND: failed to parse result: {}", e,);
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
                                    let msg =
                                        format!("C-FIND failed with status 0x{:04X}", status,);
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
                            dimse::log_scu_data(
                                &pacs.ae_title,
                                &addr,
                                "C-FIND-RSP",
                                data_value.data.len(),
                                data_value.presentation_context_id,
                            );
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

fn extract_u16_optional(obj: &InMemDicomObject<StandardDataDictionary>, tag: Tag) -> Option<u16> {
    obj.element(tag).ok()?.to_int::<u16>().ok()
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
        DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [message_id])),
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
///
/// The `query_level` determines which DICOM tags are included:
/// - `"PATIENT"`: Patient-level tags (DOB, sex, number of studies)
/// - `"SERIES"`: Series-level tags (series UID/number/description, modality, etc.)
/// - `"STUDY"` (default): Study-level tags (study date/time, description, UID, etc.)
pub(crate) fn build_cfind_identifier(
    query_level: &str,
    filters: &QueryFilters,
) -> InMemDicomObject<StandardDataDictionary> {
    let is_patient_level = query_level.eq_ignore_ascii_case("PATIENT");
    let is_series_level = query_level.eq_ignore_ascii_case("SERIES");
    let level_str = if is_patient_level {
        "PATIENT"
    } else if is_series_level {
        "SERIES"
    } else {
        "STUDY"
    };

    let mut elements: Vec<DataElement<InMemDicomObject<StandardDataDictionary>>> = vec![
        // Required: QueryRetrieveLevel
        DataElement::new(
            tags::QUERY_RETRIEVE_LEVEL,
            VR::CS,
            dicom_value!(Str, level_str),
        ),
        // PatientName (0010,0010) - common to both levels
        DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            match &filters.patient_name {
                Some(n) => dicom_value!(Str, n.as_str()),
                None => dicom_value!(),
            },
        ),
        // PatientID (0010,0020) - common to both levels
        DataElement::new(
            tags::PATIENT_ID,
            VR::LO,
            match &filters.patient_id {
                Some(id) => dicom_value!(Str, id.as_str()),
                None => dicom_value!(),
            },
        ),
    ];

    if is_patient_level {
        // PATIENT-level return keys
        elements.extend([
            // PatientBirthDate (0010,0030)
            DataElement::new(Tag(0x0010, 0x0030), VR::DA, dicom_value!()),
            // PatientSex (0010,0040)
            DataElement::new(Tag(0x0010, 0x0040), VR::CS, dicom_value!()),
            // NumberOfPatientRelatedStudies (0020,1200)
            DataElement::new(Tag(0x0020, 0x1200), VR::IS, dicom_value!()),
        ]);
    } else if is_series_level {
        // SERIES-level keys
        elements.extend([
            // StudyInstanceUID (0020,000D) - required match key for series query
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                match &filters.study_instance_uid {
                    Some(uid) => dicom_value!(Str, uid.as_str()),
                    None => dicom_value!(),
                },
            ),
            // SeriesInstanceUID (0020,000E)
            DataElement::new(Tag(0x0020, 0x000E), VR::UI, dicom_value!()),
            // Modality (0008,0060)
            DataElement::new(
                tags::MODALITY,
                VR::CS,
                match &filters.modality {
                    Some(m) => dicom_value!(Str, m.as_str()),
                    None => dicom_value!(),
                },
            ),
            // SeriesDescription (0008,103E)
            DataElement::new(Tag(0x0008, 0x103E), VR::LO, dicom_value!()),
            // SeriesNumber (0020,0011)
            DataElement::new(Tag(0x0020, 0x0011), VR::IS, dicom_value!()),
            // SeriesDate (0008,0021)
            DataElement::new(Tag(0x0008, 0x0021), VR::DA, dicom_value!()),
            // SeriesTime (0008,0031)
            DataElement::new(Tag(0x0008, 0x0031), VR::TM, dicom_value!()),
            // BodyPartExamined (0018,0015)
            DataElement::new(Tag(0x0018, 0x0015), VR::CS, dicom_value!()),
            // Laterality (0020,0060)
            DataElement::new(Tag(0x0020, 0x0060), VR::CS, dicom_value!()),
            // NumberOfSeriesRelatedInstances (0020,1209)
            DataElement::new(Tag(0x0020, 0x1209), VR::IS, dicom_value!()),
            // InstitutionName (0008,0080)
            DataElement::new(Tag(0x0008, 0x0080), VR::LO, dicom_value!()),
            // ReferringPhysicianName (0008,0090)
            DataElement::new(Tag(0x0008, 0x0090), VR::PN, dicom_value!()),
        ]);
    } else {
        // STUDY-level return keys
        elements.extend([
            // StudyDate (0008,0020)
            DataElement::new(
                tags::STUDY_DATE,
                VR::DA,
                match &filters.study_date {
                    Some(d) => dicom_value!(Str, d.as_str()),
                    None => dicom_value!(),
                },
            ),
            // StudyTime (0008,0030)
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
            // StudyDescription (0008,1030)
            DataElement::new(tags::STUDY_DESCRIPTION, VR::LO, dicom_value!()),
            // PatientBirthDate (0010,0030) - also useful at study level
            DataElement::new(Tag(0x0010, 0x0030), VR::DA, dicom_value!()),
            // PatientSex (0010,0040) - also useful at study level
            DataElement::new(Tag(0x0010, 0x0040), VR::CS, dicom_value!()),
            // StudyInstanceUID (0020,000D)
            DataElement::new(tags::STUDY_INSTANCE_UID, VR::UI, dicom_value!()),
            // StudyID (0020,0010)
            DataElement::new(Tag(0x0020, 0x0010), VR::SH, dicom_value!()),
            // NumberOfStudyRelatedSeries (0020,1206)
            DataElement::new(Tag(0x0020, 0x1206), VR::IS, dicom_value!()),
            // NumberOfStudyRelatedInstances (0020,1208)
            DataElement::new(Tag(0x0020, 0x1208), VR::IS, dicom_value!()),
        ]);
    }

    InMemDicomObject::from_element_iter(elements)
}

/// Parse a C-FIND response data object into a [`CfindResult`].
///
/// The `ts` parameter must be the transfer syntax negotiated during
/// association establishment so that response datasets are decoded
/// correctly.
///
/// The `query_level` determines which fields are required vs optional.
/// For STUDY-level and SERIES-level queries, `StudyInstanceUID` is required.
/// For SERIES-level queries, `SeriesInstanceUID` is also required.
pub(crate) fn parse_cfind_result(
    data: &[u8],
    ts: &dicom::encoding::TransferSyntax,
    query_level: &str,
) -> Result<CfindResult, String> {
    let obj = InMemDicomObject::read_dataset_with_ts(data, ts)
        .map_err(|e| format!("Failed to parse C-FIND result dataset: {}", e))?;

    let is_patient_level = query_level.eq_ignore_ascii_case("PATIENT");
    let is_series_level = query_level.eq_ignore_ascii_case("SERIES");

    // StudyInstanceUID is required for STUDY-level, optional for PATIENT-level
    let study_instance_uid = extract_string_optional(&obj, tags::STUDY_INSTANCE_UID);
    if !is_patient_level && study_instance_uid.is_none() {
        return Err("C-FIND result missing StudyInstanceUID".to_string());
    }

    let series_instance_uid = extract_string_optional(&obj, Tag(0x0020, 0x000E));
    if is_series_level && series_instance_uid.is_none() {
        return Err("C-FIND result missing SeriesInstanceUID".to_string());
    }

    Ok(CfindResult {
        // Common fields
        patient_name: extract_string_optional(&obj, tags::PATIENT_NAME),
        patient_id: extract_string_optional(&obj, tags::PATIENT_ID),

        // Study-level fields (may be None for PATIENT-level queries)
        study_date: extract_string_optional(&obj, tags::STUDY_DATE),
        study_time: extract_string_optional(&obj, tags::STUDY_TIME),
        study_description: extract_string_optional(&obj, tags::STUDY_DESCRIPTION),
        accession_number: extract_string_optional(&obj, tags::ACCESSION_NUMBER),
        study_instance_uid,
        modality: extract_string_optional(&obj, tags::MODALITY),
        modalities_in_study: extract_string_optional(&obj, tags::MODALITIES_IN_STUDY),
        number_of_series: extract_integer_optional(&obj, Tag(0x0020, 0x1206)),
        number_of_instances: if is_series_level {
            extract_integer_optional(&obj, Tag(0x0020, 0x1209))
        } else {
            extract_integer_optional(&obj, Tag(0x0020, 0x1208))
        },

        // Series-level fields
        series_instance_uid,
        series_description: extract_string_optional(&obj, Tag(0x0008, 0x103E)),
        series_number: extract_string_optional(&obj, Tag(0x0020, 0x0011)),
        series_date: extract_string_optional(&obj, Tag(0x0008, 0x0021)),
        series_time: extract_string_optional(&obj, Tag(0x0008, 0x0031)),
        body_part_examined: extract_string_optional(&obj, Tag(0x0018, 0x0015)),
        laterality: extract_string_optional(&obj, Tag(0x0020, 0x0060)),
        institution_name: extract_string_optional(&obj, Tag(0x0008, 0x0080)),
        referring_physician_name: extract_string_optional(&obj, Tag(0x0008, 0x0090)),

        // Patient-level fields
        patient_birth_date: extract_string_optional(&obj, Tag(0x0010, 0x0030)),
        patient_sex: extract_string_optional(&obj, Tag(0x0010, 0x0040)),
        number_of_patient_related_studies: extract_integer_optional(&obj, Tag(0x0020, 0x1200)),
    })
}

/// Extract the Status (0000,0900) from a C-FIND-RSP command object.
pub(crate) fn extract_status(
    obj: &InMemDicomObject<StandardDataDictionary>,
) -> Result<u16, String> {
    obj.element(tags::STATUS)
        .map_err(|_| "C-FIND-RSP missing Status tag".to_string())?
        .to_int::<u16>()
        .map_err(|e| format!("Failed to read Status value: {}", e))
}

/// Extract a string tag, returning `None` if missing or empty.
pub(crate) fn extract_string_optional(
    obj: &InMemDicomObject<StandardDataDictionary>,
    tag: Tag,
) -> Option<String> {
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

/// Extract an integer from an IS (Integer String) element, returning `None` on failure.
pub(crate) fn extract_integer_optional(
    obj: &InMemDicomObject<StandardDataDictionary>,
    tag: Tag,
) -> Option<u32> {
    match obj.element(tag) {
        Ok(elem) => match elem.to_str() {
            Ok(s) => s.trim().parse::<u32>().ok(),
            Err(_) => None,
        },
        Err(_) => None,
    }
}
