//! C-MOVE SCP handler: workstation → Bounce → remote PACS.
//!
//! Handles inbound C-MOVE-RQ requests from connected SCUs. Bounce resolves
//! the study via Aura's `move/resolve` endpoint, fetches the bytes from
//! Uhura's WADO-RS surface, then opens an outbound association to the
//! move-destination AE and forwards each instance via the existing C-STORE
//! SCU. Once the batch completes, a single final C-MOVE-RSP carries the
//! sub-operation counts and overall status back to the inbound SCU.
//!
//! ## v1 limitations
//!
//! - One final response, not interleaved Pending. The SCU sees the result at
//!   the end of the entire batch instead of per-instance progress. Per-DIMSE
//!   this is allowed; a future revision can send Pending statuses every N
//!   instances if user-visible progress on the workstation matters.
//! - The destination AE must be a configured DICOM service on the team with
//!   outbound sending enabled. Aura is the source of truth for that mapping
//!   so Bounce never has to manage destination config locally.

use crate::aura::query_api::{MoveResolveOutcome, QueryApiClient};
use crate::dimse;
use crate::query::cstore::execute_cstore;
use crate::query::models::{MoveResolveRequest, PacsService};
use crate::send::uhura_client::UhuraClient;
use crate::{log_error, log_info};
use dicom::core::{DataElement, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::encoding::TransferSyntaxIndex;
use dicom::object::{InMemDicomObject, StandardDataDictionary};
use dicom_ul::association::ServerAssociation;
use dicom_ul::pdu::{PDataValueType, Pdu};
use std::net::TcpStream;

/// Study Root Query/Retrieve Information Model - MOVE.
const STUDY_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";

/// Handle a single inbound C-MOVE-RQ.
///
/// `identifier_data` is the dataset bytes that arrived in the C-MOVE-RQ data
/// PDU; `move_destination_ae` is the AE title taken from the C-MOVE-RQ
/// command's MoveDestination element (0000,0600).
pub async fn handle_cmove(
    association: &mut ServerAssociation<TcpStream>,
    identifier_data: &[u8],
    move_destination_ae: &str,
    message_id: u16,
    presentation_context_id: u8,
    api_client: &QueryApiClient,
) -> Result<(), String> {
    let command_ts =
        dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let presentation_context = association
        .presentation_contexts()
        .iter()
        .find(|pc| pc.id == presentation_context_id)
        .cloned()
        .ok_or_else(|| {
            "C-MOVE SCP: missing presentation context for inbound association".to_string()
        })?;

    let identifier_ts = dicom_transfer_syntax_registry::TransferSyntaxRegistry
        .get(&presentation_context.transfer_syntax)
        .ok_or_else(|| {
            format!(
                "C-MOVE SCP: unsupported identifier transfer syntax {}",
                presentation_context.transfer_syntax,
            )
        })?;

    let identifier = InMemDicomObject::read_dataset_with_ts(identifier_data, identifier_ts)
        .map_err(|e| format!("C-MOVE SCP: failed to parse identifier dataset: {}", e))?;

    let study_instance_uid = match identifier.element(tags::STUDY_INSTANCE_UID) {
        Ok(elem) => elem
            .to_str()
            .map(|s| s.trim_end_matches('\0').to_string())
            .map_err(|e| format!("C-MOVE SCP: study UID extraction failed: {}", e))?,
        Err(_) => {
            log_error!("C-MOVE SCP: identifier missing StudyInstanceUID");
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xA900, // Identifier does not match SOP Class
                SubOpCounts::zero(),
            );
        }
    };

    log_info!(
        "C-MOVE SCP: request received (study={}, destination={}, message_id={})",
        study_instance_uid,
        move_destination_ae,
        message_id,
    );

    // Resolve the study + destination via Aura.
    let request = MoveResolveRequest {
        study_instance_uid: study_instance_uid.clone(),
        move_destination_ae: Some(move_destination_ae.to_string()),
    };

    let resolved = match api_client.resolve_move(&request).await {
        Ok(MoveResolveOutcome::Resolved(r)) => r,
        Ok(MoveResolveOutcome::NotFound) => {
            log_info!(
                "C-MOVE SCP: study {} not viewable in realm; replying 0xA900",
                study_instance_uid,
            );
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xA900,
                SubOpCounts::zero(),
            );
        }
        Ok(MoveResolveOutcome::DestinationUnknown) => {
            log_info!(
                "C-MOVE SCP: move destination AE {} not configured; replying 0xA801",
                move_destination_ae,
            );
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xA801,
                SubOpCounts::zero(),
            );
        }
        Err(e) => {
            log_error!("C-MOVE SCP: Aura resolve failed: {}", e);
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xA702, // Refused: out of resources, unable to perform sub-operations
                SubOpCounts::zero(),
            );
        }
    };

    let destination = match resolved.destination {
        Some(d) => d,
        None => {
            // Aura returned source-only — should not happen for a C-MOVE-RQ
            // because we sent move_destination_ae. Treat as destination
            // unknown to be safe.
            log_error!("C-MOVE SCP: Aura returned source without destination");
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xA801,
                SubOpCounts::zero(),
            );
        }
    };

    // Fetch the study from Uhura via WADO-RS.
    let uhura = UhuraClient::new();
    let instances = match uhura
        .fetch_study(
            &resolved.source.wado_base_url,
            &resolved.source.study_path,
            &resolved.source.jwt,
        )
        .await
    {
        Ok(insts) => insts,
        Err(e) => {
            log_error!(
                "C-MOVE SCP: WADO-RS fetch failed for study {}: {}",
                study_instance_uid,
                e,
            );
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xA702,
                SubOpCounts::zero(),
            );
        }
    };

    if instances.is_empty() {
        log_info!(
            "C-MOVE SCP: study {} resolved but contains no instances",
            study_instance_uid,
        );
        return send_cmove_rsp(
            association,
            presentation_context_id,
            message_id,
            &command_ts,
            0x0000,
            SubOpCounts::zero(),
        );
    }

    // Forward to the destination via C-STORE SCU.
    let pacs: PacsService = destination.clone().into();
    let calling_ae = association.client_ae_title().to_string();

    let report = match execute_cstore(&calling_ae, &pacs, &instances).await {
        Ok(r) => r,
        Err(e) => {
            log_error!(
                "C-MOVE SCP: C-STORE batch failed for study {}: {}",
                study_instance_uid,
                e,
            );
            // Even though the SCU side errored, we may have stored some
            // instances. Without per-instance progress from the SCU error
            // path, treat the whole batch as failed for sub-op accounting.
            return send_cmove_rsp(
                association,
                presentation_context_id,
                message_id,
                &command_ts,
                0xC000,
                SubOpCounts {
                    completed: 0,
                    failed: instances.len() as u16,
                    warning: 0,
                },
            );
        }
    };

    let counts = SubOpCounts {
        completed: report.successes.len() as u16,
        failed: report.failures.len() as u16,
        warning: 0,
    };

    let status = if counts.failed == 0 {
        0x0000 // Success
    } else if counts.completed > 0 {
        0xB000 // Warning: some succeeded, some failed
    } else {
        0xC000 // Unable to process
    };

    log_info!(
        "C-MOVE SCP: study {} forwarded to {}@{}:{} — completed={}, failed={}, status=0x{:04X}",
        study_instance_uid,
        destination.ae_title,
        destination.host,
        destination.port,
        counts.completed,
        counts.failed,
        status,
    );

    send_cmove_rsp(
        association,
        presentation_context_id,
        message_id,
        &command_ts,
        status,
        counts,
    )
}

/// Sub-operation counts reported in the C-MOVE-RSP command.
#[derive(Debug, Clone, Copy)]
struct SubOpCounts {
    completed: u16,
    failed: u16,
    warning: u16,
}

impl SubOpCounts {
    fn zero() -> Self {
        Self {
            completed: 0,
            failed: 0,
            warning: 0,
        }
    }
}

fn send_cmove_rsp(
    association: &mut ServerAssociation<TcpStream>,
    presentation_context_id: u8,
    message_id: u16,
    command_ts: &dicom::encoding::transfer_syntax::TransferSyntax,
    status: u16,
    counts: SubOpCounts,
) -> Result<(), String> {
    let command = build_cmove_rsp_command(message_id, status, counts);

    let mut bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut bytes, command_ts)
        .map_err(|e| format!("C-MOVE SCP: failed to serialise C-MOVE-RSP: {}", e))?;

    let pdu = Pdu::PData {
        data: vec![dicom_ul::pdu::PDataValue {
            presentation_context_id,
            value_type: PDataValueType::Command,
            is_last: true,
            data: bytes,
        }],
    };

    dimse::log_scp_response(
        association.client_ae_title(),
        "C-MOVE-RSP",
        presentation_context_id,
        message_id,
        status,
        &[
            ("completed", counts.completed.to_string()),
            ("failed", counts.failed.to_string()),
            ("warning", counts.warning.to_string()),
        ],
    );

    association
        .send(&pdu)
        .map_err(|e| format!("C-MOVE SCP: failed to send C-MOVE-RSP: {}", e))
}

fn build_cmove_rsp_command(
    message_id: u16,
    status: u16,
    counts: SubOpCounts,
) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, STUDY_ROOT_MOVE),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x8021u16]), // C-MOVE-RSP
        ),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            dicom_value!(U16, [message_id]),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0101u16]), // No dataset
        ),
        DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [status])),
        DataElement::new(
            tags::NUMBER_OF_COMPLETED_SUBOPERATIONS,
            VR::US,
            dicom_value!(U16, [counts.completed]),
        ),
        DataElement::new(
            tags::NUMBER_OF_FAILED_SUBOPERATIONS,
            VR::US,
            dicom_value!(U16, [counts.failed]),
        ),
        DataElement::new(
            tags::NUMBER_OF_WARNING_SUBOPERATIONS,
            VR::US,
            dicom_value!(U16, [counts.warning]),
        ),
    ])
}
