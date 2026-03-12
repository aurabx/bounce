//! C-FIND SCP handler for inbound queries from connected SCUs.
//!
//! When a PACS workstation or HIS sends a C-FIND request to Bounce, this
//! module:
//!
//! 1. Parses the DICOM identifier dataset to extract filter attributes.
//! 2. Posts a `FindStudyRequest` to Aura's `POST /api/bounce/find` endpoint.
//! 3. Encodes each returned study as a DICOM C-FIND-RSP Pending PDU and
//!    sends it back over the established association.
//! 4. Sends a final C-FIND-RSP Success PDU to signal completion.
//!
//! Only STUDY-level queries are supported. Series and patient-level requests
//! receive a C-FIND-RSP Failure response (0xA900).

use crate::aura::query_api::QueryApiClient;
use crate::query::cfind::extract_string_optional;
use crate::query::models::{CfindResult, FindStudyRequest, QueryFilters};
use crate::{log_error, log_info};
use dicom::core::{DataElement, Tag, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::encoding::TransferSyntaxIndex;
use dicom::object::{InMemDicomObject, StandardDataDictionary};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::association::server::ServerAssociation;
use dicom_ul::pdu::{PDataValueType, Pdu};
use std::net::TcpStream;

/// Study Root Query/Retrieve Information Model — FIND SOP class UID.
pub const STUDY_ROOT_FIND_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.2.2.1";

/// Handle an inbound C-FIND-RQ from a connected SCU.
///
/// # Arguments
///
/// * `association` — The established DICOM server association with the SCU.
/// * `identifier_data` — Raw bytes of the C-FIND identifier dataset received
///   from the SCU (encoded in the negotiated transfer syntax).
/// * `message_id` — The `MessageID` from the C-FIND-RQ command object, echoed
///   back in responses as `MessageIDBeingRespondedTo`.
/// * `presentation_context_id` — The PC ID to use when sending response PDUs.
/// * `api_client` — HTTP client for the Aura `/api/bounce/find` endpoint.
/// * `query_level` — The DICOM Query/Retrieve level: `"STUDY"`, `"SERIES"`, or
///   `"PATIENT"`. Only `"STUDY"` is proxied to Aura; others receive a failure
///   response immediately.
pub async fn handle_cfind(
    association: &mut ServerAssociation<TcpStream>,
    identifier_data: &[u8],
    message_id: u16,
    presentation_context_id: u8,
    api_client: &QueryApiClient,
    query_level: &str,
) -> Result<(), String> {
    // Only STUDY-level queries are supported.
    if !query_level.eq_ignore_ascii_case("STUDY") {
        log_error!(
            "C-FIND SCP: unsupported query level '{}' — only STUDY level is supported",
            query_level,
        );
        send_cfind_failure(
            association,
            message_id,
            presentation_context_id,
            0xA900, // Identifier does not match SOP class
        )?;
        return Ok(());
    }

    // Resolve the negotiated transfer syntax for the presentation context so we
    // can decode the identifier and encode response datasets correctly.
    let ts_uid = association
        .presentation_contexts()
        .iter()
        .find(|pc| pc.id == presentation_context_id)
        .map(|pc| pc.transfer_syntax.clone())
        .ok_or_else(|| {
            format!(
                "C-FIND SCP: no presentation context for PC ID {}",
                presentation_context_id,
            )
        })?;

    let ts = TransferSyntaxRegistry.get(&ts_uid).ok_or_else(|| {
        format!(
            "C-FIND SCP: unsupported transfer syntax negotiated: {}",
            ts_uid,
        )
    })?;

    // Parse the identifier dataset.
    let identifier = InMemDicomObject::read_dataset_with_ts(identifier_data, ts)
        .map_err(|e| format!("C-FIND SCP: failed to parse identifier dataset: {}", e))?;

    let filters = extract_filters(&identifier);

    log_info!(
        "C-FIND SCP: STUDY-level query received — filters: {:?}",
        filters,
    );

    // Query Aura.
    let find_request = FindStudyRequest::from_filters(&filters);
    let find_response = match api_client.find_studies(&find_request).await {
        Ok(resp) => resp,
        Err(e) => {
            log_error!("C-FIND SCP: Aura find request failed: {}", e);
            send_cfind_failure(
                association,
                message_id,
                presentation_context_id,
                0xA700, // Refused: out of resources
            )?;
            return Ok(());
        }
    };

    let result_count = find_response.studies.len();
    log_info!(
        "C-FIND SCP: Aura returned {} studies",
        result_count,
    );

    // Send one Pending response per study.
    for study in find_response.studies {
        let cfind_result = study.into_cfind_result();
        send_cfind_pending(
            association,
            message_id,
            presentation_context_id,
            ts,
            cfind_result,
        )?;
    }

    // Send final Success response.
    send_cfind_success(association, message_id, presentation_context_id)?;

    log_info!("C-FIND SCP: completed — sent {} results", result_count);

    Ok(())
}

/// Extract DICOM C-FIND filter attributes from the identifier dataset.
fn extract_filters(identifier: &InMemDicomObject<StandardDataDictionary>) -> QueryFilters {
    QueryFilters {
        patient_name: extract_string_optional(identifier, tags::PATIENT_NAME),
        patient_id: extract_string_optional(identifier, tags::PATIENT_ID),
        study_date: extract_string_optional(identifier, tags::STUDY_DATE),
        accession_number: extract_string_optional(identifier, tags::ACCESSION_NUMBER),
        modality: extract_string_optional(identifier, tags::MODALITIES_IN_STUDY)
            .or_else(|| extract_string_optional(identifier, tags::MODALITY)),
        study_instance_uid: extract_string_optional(identifier, tags::STUDY_INSTANCE_UID),
    }
}

/// Send a single C-FIND-RSP Pending PDU with one study result.
fn send_cfind_pending(
    association: &mut ServerAssociation<TcpStream>,
    message_id: u16,
    presentation_context_id: u8,
    ts: &dicom::encoding::TransferSyntax,
    result: CfindResult,
) -> Result<(), String> {
    let command_ts =
        dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    // Build result dataset.
    let dataset = build_study_result_dataset(&result);

    let mut dataset_bytes = Vec::new();
    dataset
        .write_dataset_with_ts(&mut dataset_bytes, ts)
        .map_err(|e| format!("C-FIND SCP: failed to serialise result dataset: {}", e))?;

    // Build Pending command (status 0xFF00).
    let command = build_cfind_rsp_command(message_id, 0xFF00);
    let mut command_bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut command_bytes, &command_ts)
        .map_err(|e| format!("C-FIND SCP: failed to serialise Pending command: {}", e))?;

    // Send data fragment first, then command — the DICOM standard requires
    // the identifier/result data to precede the command response for Pending
    // responses (PS3.7 §9.3.1.3).
    association
        .send(&Pdu::PData {
            data: vec![dicom_ul::pdu::PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Data,
                is_last: true,
                data: dataset_bytes,
            }],
        })
        .map_err(|e| format!("C-FIND SCP: failed to send result data PDU: {}", e))?;

    association
        .send(&Pdu::PData {
            data: vec![dicom_ul::pdu::PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Command,
                is_last: true,
                data: command_bytes,
            }],
        })
        .map_err(|e| format!("C-FIND SCP: failed to send Pending command PDU: {}", e))?;

    Ok(())
}

/// Send the final C-FIND-RSP Success PDU (status 0x0000, no dataset).
fn send_cfind_success(
    association: &mut ServerAssociation<TcpStream>,
    message_id: u16,
    presentation_context_id: u8,
) -> Result<(), String> {
    let command_ts =
        dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let command = build_cfind_rsp_command(message_id, 0x0000);
    let mut command_bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut command_bytes, &command_ts)
        .map_err(|e| format!("C-FIND SCP: failed to serialise Success command: {}", e))?;

    association
        .send(&Pdu::PData {
            data: vec![dicom_ul::pdu::PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Command,
                is_last: true,
                data: command_bytes,
            }],
        })
        .map_err(|e| format!("C-FIND SCP: failed to send Success command PDU: {}", e))?;

    Ok(())
}

/// Send a C-FIND-RSP Failure PDU with the given DICOM status code.
fn send_cfind_failure(
    association: &mut ServerAssociation<TcpStream>,
    message_id: u16,
    presentation_context_id: u8,
    status: u16,
) -> Result<(), String> {
    let command_ts =
        dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let command = build_cfind_rsp_command(message_id, status);
    let mut command_bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut command_bytes, &command_ts)
        .map_err(|e| format!("C-FIND SCP: failed to serialise Failure command: {}", e))?;

    association
        .send(&Pdu::PData {
            data: vec![dicom_ul::pdu::PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Command,
                is_last: true,
                data: command_bytes,
            }],
        })
        .map_err(|e| format!("C-FIND SCP: failed to send Failure command PDU: {}", e))?;

    Ok(())
}

/// Build a C-FIND-RSP command object with the given status.
///
/// The `CommandDataSetType` is set to `0x0101` (no dataset) for Success and
/// Failure responses; for Pending (`0xFF00`) the caller sends the data
/// fragment separately before calling this function.
fn build_cfind_rsp_command(
    message_id_being_responded_to: u16,
    status: u16,
) -> InMemDicomObject<StandardDataDictionary> {
    let command_data_set_type: u16 = if status == 0xFF00 || status == 0xFF01 {
        0x0000 // Dataset follows (not applicable here — we send data separately)
    } else {
        0x0101 // No dataset
    };

    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, STUDY_ROOT_FIND_SOP_CLASS),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x8020u16]), // C-FIND-RSP
        ),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            dicom_value!(U16, [message_id_being_responded_to]),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [command_data_set_type]),
        ),
        DataElement::new(
            tags::STATUS,
            VR::US,
            dicom_value!(U16, [status]),
        ),
    ])
}

/// Build a DICOM dataset for a single STUDY-level C-FIND result.
fn build_study_result_dataset(
    result: &CfindResult,
) -> InMemDicomObject<StandardDataDictionary> {
    let mut elements: Vec<DataElement<InMemDicomObject<StandardDataDictionary>>> = vec![
        DataElement::new(
            tags::QUERY_RETRIEVE_LEVEL,
            VR::CS,
            dicom_value!(Str, "STUDY"),
        ),
    ];

    macro_rules! push_str {
        ($tag:expr, $vr:expr, $opt:expr) => {
            match &$opt {
                Some(v) => elements.push(DataElement::new(
                    $tag,
                    $vr,
                    dicom_value!(Str, v.as_str()),
                )),
                None => elements.push(DataElement::new($tag, $vr, dicom_value!())),
            }
        };
    }

    macro_rules! push_is {
        ($tag:expr, $opt:expr) => {
            match $opt {
                Some(n) => elements.push(DataElement::new(
                    $tag,
                    VR::IS,
                    dicom_value!(Str, n.to_string().as_str()),
                )),
                None => elements.push(DataElement::new($tag, VR::IS, dicom_value!())),
            }
        };
    }

    push_str!(tags::STUDY_INSTANCE_UID, VR::UI, result.study_instance_uid);
    push_str!(tags::STUDY_DATE, VR::DA, result.study_date);
    push_str!(tags::STUDY_TIME, VR::TM, result.study_time);
    push_str!(tags::STUDY_DESCRIPTION, VR::LO, result.study_description);
    push_str!(tags::ACCESSION_NUMBER, VR::SH, result.accession_number);
    push_str!(tags::MODALITIES_IN_STUDY, VR::CS, result.modalities_in_study);
    push_str!(tags::PATIENT_NAME, VR::PN, result.patient_name);
    push_str!(tags::PATIENT_ID, VR::LO, result.patient_id);
    // PatientBirthDate (0010,0030)
    push_str!(Tag(0x0010, 0x0030), VR::DA, result.patient_birth_date);
    // PatientSex (0010,0040)
    push_str!(Tag(0x0010, 0x0040), VR::CS, result.patient_sex);
    // ReferringPhysicianName (0008,0090)
    push_str!(Tag(0x0008, 0x0090), VR::PN, result.referring_physician_name);
    // InstitutionName (0008,0080)
    push_str!(Tag(0x0008, 0x0080), VR::LO, result.institution_name);
    // NumberOfStudyRelatedSeries (0020,1206)
    push_is!(Tag(0x0020, 0x1206), result.number_of_series);
    // NumberOfStudyRelatedInstances (0020,1208)
    push_is!(Tag(0x0020, 0x1208), result.number_of_instances);

    InMemDicomObject::from_element_iter(elements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::models::FindStudyResult;

    // -----------------------------------------------------------------------
    // extract_filters
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_filters_empty_identifier() {
        let identifier = InMemDicomObject::from_element_iter(vec![
            DataElement::new(
                tags::QUERY_RETRIEVE_LEVEL,
                VR::CS,
                dicom_value!(Str, "STUDY"),
            ),
        ]);

        let filters = extract_filters(&identifier);

        assert!(filters.patient_name.is_none());
        assert!(filters.patient_id.is_none());
        assert!(filters.study_date.is_none());
        assert!(filters.accession_number.is_none());
        assert!(filters.modality.is_none());
        assert!(filters.study_instance_uid.is_none());
    }

    #[test]
    fn test_extract_filters_all_fields() {
        use dicom::core::DataElement;

        let identifier = InMemDicomObject::from_element_iter(vec![
            DataElement::new(tags::PATIENT_NAME, VR::PN, dicom_value!(Str, "DOE^JOHN")),
            DataElement::new(tags::PATIENT_ID, VR::LO, dicom_value!(Str, "PID-001")),
            DataElement::new(tags::STUDY_DATE, VR::DA, dicom_value!(Str, "20240615")),
            DataElement::new(tags::ACCESSION_NUMBER, VR::SH, dicom_value!(Str, "ACC-001")),
            DataElement::new(tags::MODALITIES_IN_STUDY, VR::CS, dicom_value!(Str, "CT")),
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, "1.2.3.4.5"),
            ),
        ]);

        let filters = extract_filters(&identifier);

        assert_eq!(filters.patient_name.as_deref(), Some("DOE^JOHN"));
        assert_eq!(filters.patient_id.as_deref(), Some("PID-001"));
        assert_eq!(filters.study_date.as_deref(), Some("20240615"));
        assert_eq!(filters.accession_number.as_deref(), Some("ACC-001"));
        assert_eq!(filters.modality.as_deref(), Some("CT"));
        assert_eq!(filters.study_instance_uid.as_deref(), Some("1.2.3.4.5"));
    }

    #[test]
    fn test_extract_filters_falls_back_to_modality_tag() {
        let identifier = InMemDicomObject::from_element_iter(vec![
            DataElement::new(tags::MODALITY, VR::CS, dicom_value!(Str, "MR")),
        ]);

        let filters = extract_filters(&identifier);
        assert_eq!(filters.modality.as_deref(), Some("MR"));
    }

    #[test]
    fn test_extract_filters_modalities_in_study_takes_precedence() {
        let identifier = InMemDicomObject::from_element_iter(vec![
            DataElement::new(tags::MODALITIES_IN_STUDY, VR::CS, dicom_value!(Str, "CT")),
            DataElement::new(tags::MODALITY, VR::CS, dicom_value!(Str, "MR")),
        ]);

        let filters = extract_filters(&identifier);
        // ModalitiesInStudy wins over Modality
        assert_eq!(filters.modality.as_deref(), Some("CT"));
    }

    // -----------------------------------------------------------------------
    // build_study_result_dataset
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_study_result_dataset_has_query_retrieve_level() {
        let result = CfindResult {
            study_instance_uid: Some("1.2.3".to_string()),
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            modalities_in_study: None,
            number_of_series: None,
            number_of_instances: None,
            patient_name: None,
            patient_id: None,
            patient_birth_date: None,
            patient_sex: None,
            institution_name: None,
            referring_physician_name: None,
            modality: None,
            series_instance_uid: None,
            series_description: None,
            series_number: None,
            series_date: None,
            series_time: None,
            body_part_examined: None,
            laterality: None,
            number_of_patient_related_studies: None,
        };

        let dataset = build_study_result_dataset(&result);

        let level = dataset
            .element(tags::QUERY_RETRIEVE_LEVEL)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(level.trim(), "STUDY");
    }

    #[test]
    fn test_build_study_result_dataset_encodes_study_uid() {
        let result = CfindResult {
            study_instance_uid: Some("1.2.840.999".to_string()),
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            modalities_in_study: None,
            number_of_series: None,
            number_of_instances: None,
            patient_name: Some("DOE^JOHN".to_string()),
            patient_id: Some("PID-1".to_string()),
            patient_birth_date: None,
            patient_sex: None,
            institution_name: None,
            referring_physician_name: None,
            modality: None,
            series_instance_uid: None,
            series_description: None,
            series_number: None,
            series_date: None,
            series_time: None,
            body_part_examined: None,
            laterality: None,
            number_of_patient_related_studies: None,
        };

        let dataset = build_study_result_dataset(&result);

        let uid = dataset
            .element(tags::STUDY_INSTANCE_UID)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(uid.trim(), "1.2.840.999");

        let name = dataset
            .element(tags::PATIENT_NAME)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(name.trim(), "DOE^JOHN");
    }

    // -----------------------------------------------------------------------
    // FindStudyResult::into_cfind_result — modality conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_into_cfind_result_array_modalities() {
        let result = FindStudyResult {
            study_instance_uid: Some("1.2.3".to_string()),
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            modalities_in_study: Some(serde_json::json!(["CT", "MR"])),
            number_of_series: None,
            number_of_instances: None,
            institution_name: None,
            referring_physician_name: None,
            patient_name: None,
            patient_id: None,
            patient_birth_date: None,
            patient_sex: None,
        };

        let cfind = result.into_cfind_result();
        // Array joined with backslash per DICOM convention
        assert_eq!(cfind.modalities_in_study.as_deref(), Some("CT\\MR"));
    }

    #[test]
    fn test_into_cfind_result_empty_modalities() {
        let result = FindStudyResult {
            study_instance_uid: None,
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            modalities_in_study: Some(serde_json::json!([])),
            number_of_series: None,
            number_of_instances: None,
            institution_name: None,
            referring_physician_name: None,
            patient_name: None,
            patient_id: None,
            patient_birth_date: None,
            patient_sex: None,
        };

        let cfind = result.into_cfind_result();
        assert!(cfind.modalities_in_study.is_none());
    }

    #[test]
    fn test_into_cfind_result_string_modality() {
        let result = FindStudyResult {
            study_instance_uid: None,
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            modalities_in_study: Some(serde_json::json!("CT")),
            number_of_series: None,
            number_of_instances: None,
            institution_name: None,
            referring_physician_name: None,
            patient_name: None,
            patient_id: None,
            patient_birth_date: None,
            patient_sex: None,
        };

        let cfind = result.into_cfind_result();
        assert_eq!(cfind.modalities_in_study.as_deref(), Some("CT"));
    }

    #[test]
    fn test_into_cfind_result_null_modality() {
        let result = FindStudyResult {
            study_instance_uid: None,
            study_date: None,
            study_time: None,
            study_description: None,
            accession_number: None,
            modalities_in_study: None,
            number_of_series: None,
            number_of_instances: None,
            institution_name: None,
            referring_physician_name: None,
            patient_name: None,
            patient_id: None,
            patient_birth_date: None,
            patient_sex: None,
        };

        let cfind = result.into_cfind_result();
        assert!(cfind.modalities_in_study.is_none());
    }
}
