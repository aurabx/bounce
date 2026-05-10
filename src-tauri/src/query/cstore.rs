//! C-STORE SCU (Service Class User) implementation.
//!
//! Establishes an outbound DICOM association to a remote PACS and pushes one
//! or more instances via Study Root C-STORE-RQ. This is the inverse direction
//! of [`crate::query::cmove`] — instead of asking a PACS to send us a study,
//! we send the study TO the PACS on Aurabox's behalf.
//!
//! ## Transfer syntax negotiation (v1)
//!
//! For each instance, the SOP class is taken from the file meta and
//! negotiated as an abstract syntax. For v1 we offer Explicit VR Little
//! Endian and JPEG Baseline as transfer syntaxes — see the architecture plan
//! for the rationale and the path to on-the-fly transcoding later. If the
//! destination PACS does not accept any compatible transfer syntax, the send
//! fails fast with a clear error message.
//!
//! ## Transient-failure recovery
//!
//! Mid-batch TCP drops are handled by retrying with a fresh association up to
//! [`MAX_ASSOCIATION_REOPENS`] times. Successful instances from earlier
//! attempts are remembered in an `acked` set and skipped on subsequent
//! attempts so the destination is not double-stored. Reopens only kick in
//! once at least one instance has been acknowledged — a connection that
//! never worked is treated as a configuration error and surfaced
//! immediately rather than burning the retry budget.

use crate::dimse;
use crate::query::cfind::{extract_status, extract_string_optional};
use crate::query::cmove::{describe_cmove_status, extract_u16_optional};
use crate::query::models::PacsService;
use crate::{log_error, log_info};
use dicom::core::{DataElement, Tag, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::encoding::TransferSyntaxIndex;
use dicom::object::{FileDicomObject, InMemDicomObject, StandardDataDictionary};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::association::ClientAssociationOptions;
use dicom_ul::pdu::{PDataValueType, Pdu, PresentationContextResultReason};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Maximum time to wait for the entire C-STORE batch.
///
/// A whole study can be hundreds of MB across hundreds of instances, so the
/// timeout must be generous. Tunable later if real-world traffic warrants it.
const CSTORE_TIMEOUT: Duration = Duration::from_secs(600);

/// How many times to reopen the association after a transient failure.
///
/// Combined with the exponential backoff (2 s, 4 s, 8 s) the SCU spends at
/// most ~14 s sleeping plus per-attempt work. With the 600 s overall timeout,
/// even three full attempts comfortably fit.
pub(crate) const MAX_ASSOCIATION_REOPENS: u8 = 3;

/// Transfer syntaxes Bounce will offer for every abstract syntax negotiated.
///
/// Order matters — preferred syntax first. Explicit VR Little Endian is the
/// universal baseline that almost every PACS accepts.
pub const PROPOSED_TRANSFER_SYNTAXES: &[&str] = &[
    "1.2.840.10008.1.2.1",    // Explicit VR Little Endian
    "1.2.840.10008.1.2",      // Implicit VR Little Endian
    "1.2.840.10008.1.2.4.50", // JPEG Baseline (Process 1)
];

/// Outcome of a single C-STORE-RQ for one instance.
#[derive(Debug, Clone)]
pub struct InstanceResult {
    pub sop_instance_uid: String,
    pub status: u16,
    pub error_comment: Option<String>,
}

impl InstanceResult {
    pub fn is_success(&self) -> bool {
        self.status == 0x0000
    }
}

/// Aggregate report for a C-STORE batch.
#[derive(Debug, Clone, Default)]
pub struct SendReport {
    pub successes: Vec<InstanceResult>,
    pub failures: Vec<InstanceResult>,
}

impl SendReport {
    pub fn total(&self) -> usize {
        self.successes.len() + self.failures.len()
    }
}

/// Outcome of one association attempt. Distinguishes failures the SCU can
/// reasonably recover from (TCP-level) from those it cannot (protocol-level).
#[derive(Debug)]
struct AttemptError {
    message: String,
    transient: bool,
}

impl AttemptError {
    fn transient(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), transient: true }
    }

    fn fatal(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), transient: false }
    }
}

/// Execute a batch of C-STORE-RQ operations against a remote PACS.
///
/// The instances are sent on a single association where possible. If the
/// connection drops mid-batch the SCU reopens it (up to
/// [`MAX_ASSOCIATION_REOPENS`] times) and resumes from the next un-acked
/// instance. Per-instance non-success statuses are recorded in the report
/// and do not trigger a reopen — they reflect a deliberate rejection by the
/// destination, not a network blip.
pub async fn execute_cstore(
    calling_ae: &str,
    pacs: &PacsService,
    instances: &[FileDicomObject<InMemDicomObject>],
) -> Result<SendReport, String> {
    let result = tokio::time::timeout(CSTORE_TIMEOUT, async {
        execute_cstore_inner(calling_ae, pacs, instances).await
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(format!(
            "C-STORE batch timed out after {}s sending {} instances to {}@{}:{}",
            CSTORE_TIMEOUT.as_secs(),
            instances.len(),
            pacs.ae_title,
            pacs.host,
            pacs.port,
        )),
    }
}

async fn execute_cstore_inner(
    calling_ae: &str,
    pacs: &PacsService,
    instances: &[FileDicomObject<InMemDicomObject>],
) -> Result<SendReport, String> {
    if instances.is_empty() {
        return Ok(SendReport::default());
    }

    let sop_classes = collect_sop_classes(instances)?;
    let mut report = SendReport::default();
    let mut acked: HashSet<String> = HashSet::new();
    let mut reopens: u8 = 0;

    loop {
        let remaining: Vec<&FileDicomObject<InMemDicomObject>> = instances
            .iter()
            .filter(|obj| {
                let uid = sop_instance_uid(obj).unwrap_or_default();
                !acked.contains(&uid)
                    && !report.failures.iter().any(|f| f.sop_instance_uid == uid)
            })
            .collect();

        if remaining.is_empty() {
            log_info!(
                "C-STORE: batch complete; {} succeeded, {} failed (after {} reopen(s))",
                report.successes.len(),
                report.failures.len(),
                reopens,
            );
            return Ok(report);
        }

        match attempt_batch(
            calling_ae,
            pacs,
            &sop_classes,
            &remaining,
            &mut acked,
            &mut report,
        )
        .await
        {
            Ok(()) => {
                log_info!(
                    "C-STORE: batch complete; {} succeeded, {} failed (after {} reopen(s))",
                    report.successes.len(),
                    report.failures.len(),
                    reopens,
                );
                return Ok(report);
            }
            Err(e) if e.transient
                && !acked.is_empty()
                && reopens < MAX_ASSOCIATION_REOPENS =>
            {
                reopens += 1;
                let delay_secs = 1u64 << reopens; // 2, 4, 8
                log_info!(
                    "C-STORE: association lost ({}); reopening attempt {}/{} after {}s ({}/{} instances acked so far)",
                    e.message,
                    reopens,
                    MAX_ASSOCIATION_REOPENS,
                    delay_secs,
                    acked.len(),
                    instances.len(),
                );
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }
            Err(e) => {
                let prefix = if reopens > 0 {
                    format!("after {} reopen(s): ", reopens)
                } else {
                    String::new()
                };
                return Err(format!("C-STORE: {}{}", prefix, e.message));
            }
        }
    }
}

/// Collect every distinct SOP class across the input batch. We must
/// negotiate one presentation context per abstract syntax.
fn collect_sop_classes(
    instances: &[FileDicomObject<InMemDicomObject>],
) -> Result<Vec<String>, String> {
    let mut sop_classes: Vec<String> = instances
        .iter()
        .filter_map(|obj| sop_class_uid(obj))
        .collect();
    sop_classes.sort();
    sop_classes.dedup();

    if sop_classes.is_empty() {
        return Err(
            "C-STORE: no SOP class UIDs found on any input instance; refusing to associate"
                .to_string(),
        );
    }

    Ok(sop_classes)
}

/// Run one association from establishment to release, sending the given
/// instances. Updates `acked` after each successful C-STORE-RSP and pushes
/// per-instance non-success results into `report.failures`.
///
/// On TCP-level failures (connect/send/receive errors) returns an
/// [`AttemptError`] with `transient = true` so the caller can decide whether
/// to reopen. Configuration-level problems (no PCs accepted, missing
/// presentation context for a SOP class) return `transient = false`.
async fn attempt_batch(
    calling_ae: &str,
    pacs: &PacsService,
    sop_classes: &[String],
    instances: &[&FileDicomObject<InMemDicomObject>],
    acked: &mut HashSet<String>,
    report: &mut SendReport,
) -> Result<(), AttemptError> {
    let addr = format!("{}:{}", pacs.host, pacs.port);

    log_info!(
        "C-STORE: establishing association to {} (AE: {}) from {} for {} instance(s) across {} SOP class(es)",
        addr,
        pacs.ae_title,
        calling_ae,
        instances.len(),
        sop_classes.len(),
    );

    let mut options = ClientAssociationOptions::new()
        .calling_ae_title(calling_ae)
        .called_ae_title(&pacs.ae_title);

    let proposed_ts: Vec<String> = PROPOSED_TRANSFER_SYNTAXES
        .iter()
        .map(|s| s.to_string())
        .collect();
    for sop_class in sop_classes {
        options = options.with_presentation_context(sop_class.clone(), proposed_ts.clone());
    }

    let mut association = options
        .establish_async(&addr)
        .await
        .map_err(|e| AttemptError::transient(format!(
            "failed to establish association with {}: {}",
            addr, e,
        )))?;

    log_info!(
        "C-STORE: association established with {} ({} accepted contexts)",
        pacs.ae_title,
        association.presentation_contexts().len(),
    );

    // Build a lookup from abstract syntax UID → accepted presentation context.
    // The same PC can be used for every instance whose SOP class matches.
    // PC ids are assigned in the order with_presentation_context() was called,
    // starting at 1, so we map each accepted context back to its SOP class
    // by index.
    let mut by_abstract_syntax: HashMap<String, AcceptedPc> = HashMap::new();
    for pc in association.presentation_contexts() {
        if pc.reason != PresentationContextResultReason::Acceptance {
            continue;
        }
        let idx = (pc.id as usize).saturating_sub(1);
        if let Some(sop) = sop_classes.get(idx) {
            by_abstract_syntax.insert(
                sop.clone(),
                AcceptedPc {
                    id: pc.id,
                    transfer_syntax: pc.transfer_syntax.clone(),
                },
            );
        }
    }

    if by_abstract_syntax.is_empty() {
        let _ = association.release().await;
        return Err(AttemptError::fatal(format!(
            "PACS {} did not accept any presentation context",
            pacs.ae_title,
        )));
    }

    let command_ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
    let mut message_id: u16 = 1;

    for (idx, obj) in instances.iter().enumerate() {
        let sop_class = match sop_class_uid(obj) {
            Some(s) => s,
            None => {
                log_error!("C-STORE: instance {} has no SOP class UID; skipping", idx);
                continue;
            }
        };
        let sop_instance = sop_instance_uid(obj).unwrap_or_default();

        let pc = match by_abstract_syntax.get(&sop_class) {
            Some(pc) => pc.clone(),
            None => {
                let msg = format!(
                    "no accepted presentation context for SOP class {}",
                    sop_class,
                );
                log_error!("C-STORE: instance {} ({}): {}", idx, sop_instance, msg);
                report.failures.push(InstanceResult {
                    sop_instance_uid: sop_instance.clone(),
                    status: 0xC000,
                    error_comment: Some(msg),
                });
                continue;
            }
        };

        let negotiated_ts = match TransferSyntaxRegistry.get(&pc.transfer_syntax) {
            Some(ts) => ts,
            None => {
                let msg = format!(
                    "unsupported transfer syntax {} negotiated for SOP class {}",
                    pc.transfer_syntax, sop_class,
                );
                log_error!("C-STORE: instance {} ({}): {}", idx, sop_instance, msg);
                report.failures.push(InstanceResult {
                    sop_instance_uid: sop_instance.clone(),
                    status: 0xC000,
                    error_comment: Some(msg),
                });
                continue;
            }
        };

        // Serialize the dataset (the instance bytes) using the negotiated TS.
        let mut data_bytes = Vec::new();
        if let Err(e) = obj.write_dataset_with_ts(&mut data_bytes, negotiated_ts) {
            let msg = format!("failed to serialize instance: {}", e);
            log_error!("C-STORE: instance {} ({}): {}", idx, sop_instance, msg);
            report.failures.push(InstanceResult {
                sop_instance_uid: sop_instance.clone(),
                status: 0xC000,
                error_comment: Some(msg),
            });
            continue;
        }

        // Build the C-STORE-RQ command for this instance.
        let command = build_cstore_command(message_id, &sop_class, &sop_instance);
        let mut command_bytes = Vec::new();
        if let Err(e) = command.write_dataset_with_ts(&mut command_bytes, &command_ts) {
            let msg = format!("failed to serialize C-STORE-RQ command: {}", e);
            log_error!("C-STORE: instance {} ({}): {}", idx, sop_instance, msg);
            report.failures.push(InstanceResult {
                sop_instance_uid: sop_instance.clone(),
                status: 0xC000,
                error_comment: Some(msg),
            });
            continue;
        }

        dimse::log_scu_request(
            &pacs.ae_title,
            &addr,
            "C-STORE-RQ",
            message_id,
            &[
                ("sop_class_uid", sop_class.clone()),
                ("sop_instance_uid", sop_instance.clone()),
                ("transfer_syntax", pc.transfer_syntax.clone()),
            ],
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
            return Err(AttemptError::transient(format!(
                "failed to send C-STORE-RQ command for {}: {}",
                sop_instance, e,
            )));
        }

        let data_pdu = Pdu::PData {
            data: vec![dicom_ul::pdu::PDataValue {
                presentation_context_id: pc.id,
                value_type: PDataValueType::Data,
                is_last: true,
                data: data_bytes,
            }],
        };
        if let Err(e) = association.send(&data_pdu).await {
            let _ = association.abort().await;
            return Err(AttemptError::transient(format!(
                "failed to send C-STORE-RQ dataset for {}: {}",
                sop_instance, e,
            )));
        }

        // Receive the C-STORE-RSP for this instance.
        let result = match receive_cstore_response(&mut association, &command_ts, &sop_instance)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = association.abort().await;
                return Err(e);
            }
        };

        dimse::log_scu_response(
            &pacs.ae_title,
            &addr,
            "C-STORE-RSP",
            Some(message_id),
            result.status,
            &[
                ("sop_instance_uid", result.sop_instance_uid.clone()),
                (
                    "error_comment",
                    result.error_comment.clone().unwrap_or_default(),
                ),
            ],
        );

        if result.is_success() {
            acked.insert(result.sop_instance_uid.clone());
            report.successes.push(result);
        } else {
            // Non-success status is the destination deliberately rejecting
            // this instance — record it and move on without retrying.
            report.failures.push(result);
        }

        message_id = message_id.wrapping_add(1);
        if message_id == 0 {
            message_id = 1;
        }
    }

    if let Err(e) = association.release().await {
        log_error!("C-STORE: failed to release association cleanly: {}", e);
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct AcceptedPc {
    id: u8,
    transfer_syntax: String,
}

async fn receive_cstore_response(
    association: &mut dicom_ul::association::ClientAssociation<tokio::net::TcpStream>,
    command_ts: &dicom::encoding::transfer_syntax::TransferSyntax,
    sop_instance: &str,
) -> Result<InstanceResult, AttemptError> {
    loop {
        let pdu = association.receive().await.map_err(|e| {
            AttemptError::transient(format!(
                "failed to receive C-STORE-RSP for {}: {}",
                sop_instance, e,
            ))
        })?;

        match pdu {
            Pdu::PData { ref data } => {
                for data_value in data {
                    if data_value.value_type == PDataValueType::Command && data_value.is_last {
                        let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                            data_value.data.as_slice(),
                            command_ts,
                        )
                        .map_err(|e| {
                            AttemptError::fatal(format!(
                                "failed to parse C-STORE-RSP command for {}: {}",
                                sop_instance, e,
                            ))
                        })?;

                        let status = extract_status(&cmd_obj)
                            .map_err(|e| AttemptError::fatal(e.to_string()))?;
                        let error_comment =
                            extract_string_optional(&cmd_obj, Tag(0x0000, 0x0902));

                        if status == 0xFF00 {
                            // Pending — should not occur for C-STORE per the
                            // standard, but handle defensively by waiting for
                            // a final status.
                            log_info!(
                                "C-STORE: PACS sent Pending status for {}; waiting for final",
                                sop_instance,
                            );
                            continue;
                        }

                        // Sub-operation counts and other diagnostic fields are
                        // sometimes returned; surface anything unexpected.
                        let _maybe_failed_sop_uid =
                            extract_u16_optional(&cmd_obj, tags::NUMBER_OF_FAILED_SUBOPERATIONS);

                        if !is_success_or_warning(status) {
                            log_error!(
                                "C-STORE: failure status 0x{:04X} ({}) for {}: {:?}",
                                status,
                                describe_cmove_status(status),
                                sop_instance,
                                error_comment,
                            );
                        }

                        return Ok(InstanceResult {
                            sop_instance_uid: sop_instance.to_string(),
                            status,
                            error_comment,
                        });
                    }
                }
            }
            Pdu::ReleaseRQ => {
                let _ = association.send(&Pdu::ReleaseRP).await;
                return Err(AttemptError::transient(format!(
                    "PACS released association before responding for {}",
                    sop_instance,
                )));
            }
            Pdu::AbortRQ { source } => {
                return Err(AttemptError::transient(format!(
                    "PACS aborted association while sending {}: {:?}",
                    sop_instance, source,
                )));
            }
            _ => {
                // Ignore non-data PDUs and continue waiting.
            }
        }
    }
}

/// Build the C-STORE-RQ command object.
fn build_cstore_command(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) -> InMemDicomObject<StandardDataDictionary> {
    let affected_sop_instance_tag = Tag(0x0000, 0x1000);

    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, sop_class_uid),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            dicom_value!(U16, [0x0001]), // C-STORE-RQ
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
            dicom_value!(U16, [0x0001]), // Dataset present
        ),
        DataElement::new(
            affected_sop_instance_tag,
            VR::UI,
            dicom_value!(Str, sop_instance_uid),
        ),
    ])
}

fn sop_class_uid(obj: &FileDicomObject<InMemDicomObject>) -> Option<String> {
    let meta = obj.meta();
    Some(
        meta.media_storage_sop_class_uid()
            .trim_end_matches('\0')
            .to_string(),
    )
}

fn sop_instance_uid(obj: &FileDicomObject<InMemDicomObject>) -> Option<String> {
    let meta = obj.meta();
    Some(
        meta.media_storage_sop_instance_uid()
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// 0x0000 = Success; 0xB000-0xBFFF range are warnings — both indicate the
/// instance was stored successfully from the SCU's perspective.
fn is_success_or_warning(status: u16) -> bool {
    status == 0x0000 || (0xB000..=0xBFFF).contains(&status)
}
