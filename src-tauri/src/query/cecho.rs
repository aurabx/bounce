//! C-ECHO SCU (verification) implementation.
//!
//! Opens an outbound DICOM association to a remote PACS, negotiates the
//! Verification SOP Class, sends a C-ECHO-RQ, and waits for the matching
//! C-ECHO-RSP. Used by the PACS tab to test connectivity to a configured
//! service on demand.
//!
//! C-ECHO carries no dataset — only a command PDU is sent, and only a
//! command PDU is expected back.

use crate::dimse;
use crate::query::cfind::{extract_status, extract_string_optional};
use crate::query::models::PacsService;
use crate::{log_error, log_info};
use dicom::core::{DataElement, Tag, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::object::{InMemDicomObject, StandardDataDictionary};
use dicom_ul::association::ClientAssociationOptions;
use dicom_ul::pdu::{PDataValueType, Pdu, PresentationContextResultReason};
use std::time::{Duration, Instant};

/// Verification SOP Class UID — the abstract syntax negotiated for C-ECHO.
const VERIFICATION_SOP_CLASS: &str = "1.2.840.10008.1.1";

/// Wall-clock cap on a single C-ECHO round trip including association
/// setup. Generous so very slow PACS still report rather than hang the UI.
const CECHO_TIMEOUT: Duration = Duration::from_secs(15);

/// Transfer syntaxes proposed for the Verification PC. Implicit VR LE is the
/// universal baseline mandated by the DICOM standard for verification.
const PROPOSED_TRANSFER_SYNTAXES: &[&str] = &[
    "1.2.840.10008.1.2",   // Implicit VR Little Endian
    "1.2.840.10008.1.2.1", // Explicit VR Little Endian
];

/// Successful C-ECHO outcome. A non-success status is converted to `Err` by
/// [`execute_cecho`], so callers only see the latency on success.
#[derive(Debug, Clone)]
pub struct EchoReport {
    pub latency_ms: u64,
}

/// Run a C-ECHO against the given PACS service and return the round-trip
/// latency. Failures (TCP errors, association rejection, non-success status,
/// timeout) are returned as `Err(message)` so the caller can surface them
/// directly to the user.
pub async fn execute_cecho(calling_ae: &str, pacs: &PacsService) -> Result<EchoReport, String> {
    let started = Instant::now();
    let result =
        tokio::time::timeout(CECHO_TIMEOUT, execute_cecho_inner(calling_ae, pacs, started)).await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(format!(
            "C-ECHO timed out after {}s contacting {}@{}:{}",
            CECHO_TIMEOUT.as_secs(),
            pacs.ae_title,
            pacs.host,
            pacs.port,
        )),
    }
}

async fn execute_cecho_inner(
    calling_ae: &str,
    pacs: &PacsService,
    started: Instant,
) -> Result<EchoReport, String> {
    let addr = format!("{}:{}", pacs.host, pacs.port);

    log_info!(
        "C-ECHO: establishing association to {} (AE: {}) from {}",
        addr,
        pacs.ae_title,
        calling_ae,
    );

    let proposed_ts: Vec<String> = PROPOSED_TRANSFER_SYNTAXES
        .iter()
        .map(|s| s.to_string())
        .collect();

    let options = ClientAssociationOptions::new()
        .calling_ae_title(calling_ae)
        .called_ae_title(&pacs.ae_title)
        .with_presentation_context(VERIFICATION_SOP_CLASS.to_string(), proposed_ts);

    let mut association = options.establish_async(&addr).await.map_err(|e| {
        format!(
            "C-ECHO: failed to establish association with {}: {}",
            addr, e,
        )
    })?;

    let pc = association
        .presentation_contexts()
        .iter()
        .find(|pc| pc.reason == PresentationContextResultReason::Acceptance)
        .cloned();

    let pc = match pc {
        Some(pc) => pc,
        None => {
            let _ = association.abort().await;
            return Err(format!(
                "C-ECHO: PACS {} rejected the Verification presentation context",
                pacs.ae_title,
            ));
        }
    };

    let command_ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let message_id: u16 = 1;
    let command = build_cecho_command(message_id);

    let mut command_bytes = Vec::new();
    command
        .write_dataset_with_ts(&mut command_bytes, &command_ts)
        .map_err(|e| format!("C-ECHO: failed to serialize C-ECHO-RQ command: {}", e))?;

    dimse::log_scu_request(
        &pacs.ae_title,
        &addr,
        "C-ECHO-RQ",
        message_id,
        &[("sop_class_uid", VERIFICATION_SOP_CLASS.to_string())],
    );

    let command_pdu = Pdu::PData {
        data: vec![dicom_ul::pdu::PDataValue {
            presentation_context_id: pc.id,
            value_type: PDataValueType::Command,
            is_last: true,
            data: command_bytes,
        }],
    };

    if let Err(e) = association.send(&command_pdu).await {
        let _ = association.abort().await;
        return Err(format!("C-ECHO: failed to send command: {}", e));
    }

    let response = receive_cecho_response(&mut association, &command_ts).await;

    if let Err(e) = association.release().await {
        log_error!("C-ECHO: failed to release association cleanly: {}", e);
    }

    let result = response?;
    let latency_ms = started.elapsed().as_millis() as u64;

    dimse::log_scu_response(
        &pacs.ae_title,
        &addr,
        "C-ECHO-RSP",
        Some(message_id),
        result.status,
        &[("latency_ms", latency_ms.to_string())],
    );

    if result.status != 0x0000 {
        return Err(format!(
            "C-ECHO: PACS {} returned non-success status 0x{:04X}{}",
            pacs.ae_title,
            result.status,
            result
                .error_comment
                .map(|c| format!(" ({})", c))
                .unwrap_or_default(),
        ));
    }

    Ok(EchoReport { latency_ms })
}

struct CechoResponse {
    status: u16,
    error_comment: Option<String>,
}

async fn receive_cecho_response(
    association: &mut dicom_ul::association::ClientAssociation<tokio::net::TcpStream>,
    command_ts: &dicom::encoding::transfer_syntax::TransferSyntax,
) -> Result<CechoResponse, String> {
    loop {
        let pdu = association
            .receive()
            .await
            .map_err(|e| format!("C-ECHO: failed to receive response: {}", e))?;

        match pdu {
            Pdu::PData { ref data } => {
                for data_value in data {
                    if data_value.value_type == PDataValueType::Command && data_value.is_last {
                        let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                            data_value.data.as_slice(),
                            command_ts,
                        )
                        .map_err(|e| format!("C-ECHO: failed to parse RSP command: {}", e))?;

                        let status = extract_status(&cmd_obj)?;
                        let error_comment =
                            extract_string_optional(&cmd_obj, Tag(0x0000, 0x0902));

                        return Ok(CechoResponse {
                            status,
                            error_comment,
                        });
                    }
                }
            }
            Pdu::ReleaseRQ => {
                let _ = association.send(&Pdu::ReleaseRP).await;
                return Err("C-ECHO: PACS released association before responding".to_string());
            }
            Pdu::AbortRQ { source } => {
                return Err(format!(
                    "C-ECHO: PACS aborted association: {:?}",
                    source,
                ));
            }
            _ => {}
        }
    }
}

/// Build a C-ECHO-RQ command object. There is no dataset — Command Data Set
/// Type is set to 0x0101 (no dataset present).
fn build_cecho_command(message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, VERIFICATION_SOP_CLASS),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x0030]), // C-ECHO-RQ
        ),
        DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [message_id])),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0101]), // No dataset
        ),
    ])
}
