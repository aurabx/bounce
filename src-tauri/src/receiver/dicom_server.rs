use crate::aura::query_api::QueryApiClient;
use crate::db::database::Database;
use crate::dimse;
use crate::receiver::cfind_handler;
use crate::receiver::cmove_handler;
use crate::receiver::metadata::Metadata;
use crate::transmitter::transmission::QueueUpload;
use crate::{load_config, log_error, log_info, receiver, store};
use dicom::core::{DataElement, Tag, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::encoding::TransferSyntaxIndex;
use dicom::object::{FileMetaTableBuilder, InMemDicomObject, StandardDataDictionary};
use dicom::transfer_syntax::TransferSyntaxRegistry;
use dicom_ul::{pdu::PDataValueType, Pdu};
use receiver::enums::{ABSTRACT_SYNTAXES, STUDY_ROOT_FIND, STUDY_ROOT_MOVE};
use snafu::{OptionExt, Report, ResultExt, Whatever};
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use store::config::Config;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone)]
pub struct DICOMServer {
    config: Arc<Config>,
    app_handle: Option<AppHandle>,
    database: Database,
}

#[derive(Clone)]
#[allow(clippy::enum_variant_names)]
enum PendingDimseCommand {
    CStore {
        message_id: u16,
        sop_class_uid: String,
        sop_instance_uid: String,
    },
    CFind {
        message_id: u16,
        presentation_context_id: u8,
        query_level: String,
    },
    CMove {
        message_id: u16,
        presentation_context_id: u8,
        move_destination_ae: String,
    },
}

impl DICOMServer {
    pub fn new(config: Config, app_handle: AppHandle) -> Self {
        let database = app_handle.state::<Database>().inner().clone();
        Self {
            config: Arc::new(config.clone()),
            app_handle: Some(app_handle),
            database,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(config: Config, database: Database) -> Self {
        Self {
            config: Arc::new(config),
            app_handle: None,
            database,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let server = Arc::new(self.clone());
        let port = server.config.port;
        let out_dir = server.config.get_base_dir().clone();
        let path = PathBuf::from(&out_dir);

        fs::create_dir_all(&out_dir).unwrap_or_else(|e| {
            log_error!("Could not create output directory: {}", e);
            std::process::exit(-2);
        });

        // Bind the listener
        let ip_address: Result<Ipv4Addr, _> = server.config.ip_address.clone().parse();
        // let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), port);
        let listen_addr = SocketAddrV4::new(ip_address.unwrap(), port);
        let listener = tokio::net::TcpListener::bind(listen_addr).await?;
        log_info!("listening on: tcp://{}", listen_addr);

        // Convert to tokio listener - this lets us use accept_async
        // let listener = tokio::net::TcpListener::from_std(listener)?;

        let current_path = path.clone();

        // Instead of spawning a task here, we run the loop directly
        // This allows the task to be cancelable from the outside
        loop {
            // Accept connections with timeout to allow checking for shutdown
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(1), // Check every second
                listener.accept(),
            )
            .await
            {
                Ok(Ok((scu_stream, _addr))) => {
                    // Convert to std TcpStream for your DICOM library
                    let std_stream = scu_stream.into_std()?;

                    std_stream.set_nonblocking(false)?;

                    // Process the connection
                    if let Err(e) = server.run_store_sync(std_stream, &current_path).await {
                        log_error!("{}", Report::from_error(e));
                    }
                }
                Ok(Err(e)) => {
                    log_error!("Error accepting connection: {}", e);
                }
                Err(_) => {
                    // Timeout - just continue and check for shutdown signal
                    continue;
                }
            }
        }
    }

    pub async fn run_store_sync(
        &self,
        scu_stream: TcpStream,
        out_dir: &std::path::Path,
    ) -> Result<(), Whatever> {
        let server = Arc::new(self.clone());
        let calling_ae_title = server.config.ae_title.clone();

        let verbose = true;
        let strict = false;
        let uncompressed_only = false;
        let promiscuous = true;
        let max_pdu_length = 16384;

        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        let mut instance_buffer: Vec<u8> = Vec::with_capacity(1024 * 1024);
        let mut pending_command: Option<PendingDimseCommand> = None;

        let mut options = dicom_ul::association::ServerAssociationOptions::new()
            .accept_any()
            .ae_title(calling_ae_title)
            .strict(strict)
            .promiscuous(promiscuous);

        if uncompressed_only {
            options = options
                .with_transfer_syntax("1.2.840.10008.1.2")
                .with_transfer_syntax("1.2.840.10008.1.2.1");
        } else {
            for ts in TransferSyntaxRegistry.iter() {
                if !ts.is_unsupported() {
                    options = options.with_transfer_syntax(ts.uid());
                }
            }
        };

        for uid in ABSTRACT_SYNTAXES {
            options = options.with_abstract_syntax(*uid);
        }

        // Accept Study Root C-FIND so connected SCUs can query Aura's
        // study database via Bounce.
        options = options.with_abstract_syntax(STUDY_ROOT_FIND);

        // Accept Study Root C-MOVE so connected SCUs can pull studies from
        // Aurabox. Bounce fetches the bytes from Uhura via WADO-RS and
        // forwards them to the move-destination AE via C-STORE SCU.
        options = options.with_abstract_syntax(STUDY_ROOT_MOVE);

        let mut association = options
            .establish(scu_stream)
            .whatever_context("could not establish association")?;

        log_info!("New association from {}", association.client_ae_title());

        // Send message to JavaScript
        if let Some(app_handle) = &self.app_handle {
            app_handle
                .emit(
                    "log",
                    format!("New association from {}", association.client_ae_title()),
                )
                .unwrap_or_else(|e| {
                    println!("Failed to emit log event: {}", e);
                });
        }

        loop {
            match association.receive() {
                Ok(mut pdu) => {
                    if verbose {
                        log_info!("scu ----> scp: {}", pdu.short_description());
                    }
                    match pdu {
                        Pdu::PData { ref mut data } => {
                            if data.is_empty() {
                                log_info!("Ignoring empty PData PDU");
                                continue;
                            }

                            for data_value in data {
                                if data_value.value_type == PDataValueType::Data
                                    && !data_value.is_last
                                {
                                    instance_buffer.append(&mut data_value.data);
                                } else if data_value.value_type == PDataValueType::Command
                                    && data_value.is_last
                                {
                                    // commands are always in implict VR LE
                                    let ts =
                                        dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                                            .erased();
                                    let data_value = &data_value;
                                    let v = &data_value.data;

                                    let obj =
                                        InMemDicomObject::read_dataset_with_ts(v.as_slice(), &ts)
                                            .whatever_context(
                                            "failed to read incoming DICOM command",
                                        )?;

                                    let command_field =
                                        Self::extract_int_tag(&obj, tags::COMMAND_FIELD)?;

                                    let request_name = dimse::describe_request(command_field);
                                    let incoming_message_id =
                                        Self::extract_int_tag_optional(&obj, tags::MESSAGE_ID)
                                            .unwrap_or(1);

                                    if command_field == 0x0030 {
                                        pending_command = None;
                                        dimse::log_scp_request(
                                            association.client_ae_title(),
                                            request_name,
                                            data_value.presentation_context_id,
                                            incoming_message_id,
                                            &[(
                                                "affected_sop_class_uid",
                                                dimse::format_optional_str(
                                                    Self::extract_string_tag_optional(
                                                        &obj,
                                                        tags::AFFECTED_SOP_CLASS_UID,
                                                    )
                                                    .as_deref(),
                                                ),
                                            )],
                                        );

                                        // Handle C-ECHO-RQ
                                        let cecho_response =
                                            self.create_cecho_response(incoming_message_id);
                                        let mut cecho_data = Vec::new();

                                        cecho_response
                                            .write_dataset_with_ts(&mut cecho_data, &ts)
                                            .whatever_context(
                                                "could not write C-ECHO response object",
                                            )?;

                                        let pdu_response = Pdu::PData {
                                            data: vec![dicom_ul::pdu::PDataValue {
                                                presentation_context_id: data_value
                                                    .presentation_context_id,
                                                value_type: PDataValueType::Command,
                                                is_last: true,
                                                data: cecho_data,
                                            }],
                                        };
                                        dimse::log_scp_response(
                                            association.client_ae_title(),
                                            "C-ECHO-RSP",
                                            data_value.presentation_context_id,
                                            incoming_message_id,
                                            0x0000,
                                            &[],
                                        );
                                        association.send(&pdu_response).whatever_context(
                                            "failed to send C-ECHO response object to SCU",
                                        )?;
                                    } else if command_field == 0x0020 {
                                        let find_msg_id =
                                            Self::extract_int_tag(&obj, tags::MESSAGE_ID)
                                                .unwrap_or(1);
                                        let query_level = Self::extract_string_tag_optional(
                                            &obj,
                                            tags::QUERY_RETRIEVE_LEVEL,
                                        )
                                        .unwrap_or_else(|| "STUDY".to_string());

                                        dimse::log_scp_request(
                                            association.client_ae_title(),
                                            request_name,
                                            data_value.presentation_context_id,
                                            find_msg_id,
                                            &[
                                                ("query_level", query_level.clone()),
                                                (
                                                    "priority",
                                                    dimse::format_optional_u16(
                                                        Self::extract_int_tag_optional(
                                                            &obj,
                                                            tags::PRIORITY,
                                                        ),
                                                    ),
                                                ),
                                                (
                                                    "affected_sop_class_uid",
                                                    dimse::format_optional_str(
                                                        Self::extract_string_tag_optional(
                                                            &obj,
                                                            tags::AFFECTED_SOP_CLASS_UID,
                                                        )
                                                        .as_deref(),
                                                    ),
                                                ),
                                            ],
                                        );

                                        pending_command = Some(PendingDimseCommand::CFind {
                                            message_id: find_msg_id,
                                            presentation_context_id: data_value
                                                .presentation_context_id,
                                            query_level,
                                        });
                                    } else if command_field == 0x0001 {
                                        let message_id = incoming_message_id;
                                        let sop_class_uid = Self::extract_string_tag(
                                            &obj,
                                            tags::AFFECTED_SOP_CLASS_UID,
                                        )?;
                                        let sop_instance_uid = Self::extract_string_tag(
                                            &obj,
                                            tags::AFFECTED_SOP_INSTANCE_UID,
                                        )?;

                                        dimse::log_scp_request(
                                            association.client_ae_title(),
                                            request_name,
                                            data_value.presentation_context_id,
                                            message_id,
                                            &[
                                                (
                                                    "priority",
                                                    dimse::format_optional_u16(
                                                        Self::extract_int_tag_optional(
                                                            &obj,
                                                            tags::PRIORITY,
                                                        ),
                                                    ),
                                                ),
                                                ("affected_sop_class_uid", sop_class_uid.clone()),
                                                (
                                                    "affected_sop_instance_uid",
                                                    sop_instance_uid.clone(),
                                                ),
                                            ],
                                        );
                                        pending_command = Some(PendingDimseCommand::CStore {
                                            message_id,
                                            sop_class_uid: sop_class_uid.clone(),
                                            sop_instance_uid: sop_instance_uid.clone(),
                                        });
                                    } else if command_field == 0x0021 {
                                        // C-MOVE-RQ — workstation asking
                                        // Bounce to forward a study to a
                                        // named destination AE.
                                        let move_msg_id = incoming_message_id;
                                        let move_destination_ae =
                                            Self::extract_string_tag_optional(
                                                &obj,
                                                Tag(0x0000, 0x0600),
                                            )
                                            .map(|s| s.trim_end_matches('\0').to_string())
                                            .unwrap_or_default();

                                        dimse::log_scp_request(
                                            association.client_ae_title(),
                                            request_name,
                                            data_value.presentation_context_id,
                                            move_msg_id,
                                            &[
                                                (
                                                    "move_destination",
                                                    move_destination_ae.clone(),
                                                ),
                                                (
                                                    "priority",
                                                    dimse::format_optional_u16(
                                                        Self::extract_int_tag_optional(
                                                            &obj,
                                                            tags::PRIORITY,
                                                        ),
                                                    ),
                                                ),
                                                (
                                                    "affected_sop_class_uid",
                                                    dimse::format_optional_str(
                                                        Self::extract_string_tag_optional(
                                                            &obj,
                                                            tags::AFFECTED_SOP_CLASS_UID,
                                                        )
                                                        .as_deref(),
                                                    ),
                                                ),
                                            ],
                                        );

                                        pending_command = Some(PendingDimseCommand::CMove {
                                            message_id: move_msg_id,
                                            presentation_context_id: data_value
                                                .presentation_context_id,
                                            move_destination_ae,
                                        });
                                    } else {
                                        pending_command = None;
                                        dimse::log_scp_request(
                                            association.client_ae_title(),
                                            request_name,
                                            data_value.presentation_context_id,
                                            incoming_message_id,
                                            &[],
                                        );
                                    }
                                    instance_buffer.clear();
                                } else if data_value.value_type == PDataValueType::Data
                                    && data_value.is_last
                                {
                                    instance_buffer.append(&mut data_value.data);

                                    let Some(current_command) = pending_command.clone() else {
                                        log_info!(
                                            "DIMSE SCP: received data fragment with no pending command pc_id={} len={}",
                                            data_value.presentation_context_id,
                                            instance_buffer.len(),
                                        );
                                        instance_buffer.clear();
                                        continue;
                                    };

                                    let presentation_context = association
                                        .presentation_contexts()
                                        .iter()
                                        .find(|pc| pc.id == data_value.presentation_context_id)
                                        .whatever_context("missing presentation context")?;
                                    let ts = &presentation_context.transfer_syntax;

                                    let transfer_syntax = TransferSyntaxRegistry
                                        .get(ts)
                                        .whatever_context(format!(
                                            "unsupported transfer syntax: {}",
                                            ts
                                        ))?;
                                    let obj = InMemDicomObject::read_dataset_with_ts(
                                        instance_buffer.as_slice(),
                                        transfer_syntax,
                                    )
                                    .whatever_context("failed to read DICOM data object")?;

                                    match current_command {
                                        PendingDimseCommand::CFind {
                                            message_id,
                                            presentation_context_id,
                                            query_level,
                                        } => {
                                            let api_client = if let Some(ref app_handle) =
                                                self.app_handle
                                            {
                                                let cfg = load_config(app_handle.clone());
                                                QueryApiClient::new(
                                                    cfg.get_api_endpoint(),
                                                    cfg.api_key,
                                                )
                                            } else {
                                                log_error!("C-FIND SCP: no app handle — cannot build API client");
                                                instance_buffer.clear();
                                                pending_command = None;
                                                continue;
                                            };

                                            if let Err(e) = cfind_handler::handle_cfind(
                                                &mut association,
                                                instance_buffer.as_slice(),
                                                message_id,
                                                presentation_context_id,
                                                &api_client,
                                                &query_level,
                                            )
                                            .await
                                            {
                                                log_error!("C-FIND SCP: handler error: {}", e);
                                            }
                                        }
                                        PendingDimseCommand::CMove {
                                            message_id,
                                            presentation_context_id,
                                            move_destination_ae,
                                        } => {
                                            let api_client = if let Some(ref app_handle) =
                                                self.app_handle
                                            {
                                                let cfg = load_config(app_handle.clone());
                                                QueryApiClient::new(
                                                    cfg.get_api_endpoint(),
                                                    cfg.api_key,
                                                )
                                            } else {
                                                log_error!("C-MOVE SCP: no app handle — cannot build API client");
                                                instance_buffer.clear();
                                                pending_command = None;
                                                continue;
                                            };

                                            if let Err(e) = cmove_handler::handle_cmove(
                                                &mut association,
                                                instance_buffer.as_slice(),
                                                &move_destination_ae,
                                                message_id,
                                                presentation_context_id,
                                                &api_client,
                                            )
                                            .await
                                            {
                                                log_error!("C-MOVE SCP: handler error: {}", e);
                                            }
                                        }
                                        PendingDimseCommand::CStore {
                                            message_id,
                                            sop_class_uid,
                                            sop_instance_uid,
                                        } => {
                                            // Extract StudyInstanceUID
                                            let study_uid = Self::extract_string_tag(
                                                &obj,
                                                tags::STUDY_INSTANCE_UID,
                                            )?;
                                            let series_uid = Self::extract_string_tag(
                                                &obj,
                                                tags::SERIES_INSTANCE_UID,
                                            )?;

                                            dimse::log_scp_payload(
                                                "C-STORE-RQ",
                                                &[
                                                    ("study_instance_uid", study_uid.clone()),
                                                    ("series_instance_uid", series_uid.clone()),
                                                    ("sop_instance_uid", sop_instance_uid.clone()),
                                                ],
                                            );

                                            let file_meta = FileMetaTableBuilder::new()
                                                .transfer_syntax(ts)
                                                .build()
                                                .whatever_context(
                                                    "failed to build DICOM meta file information",
                                                )?;

                                            let mut file_path = out_dir.to_path_buf();
                                            file_path
                                                .push(Self::sanitize_uid_component(&study_uid)?);
                                            file_path
                                                .push(Self::sanitize_uid_component(&series_uid)?);
                                            let series_dir = file_path.clone();

                                            if !series_dir.exists() {
                                                fs::create_dir_all(&series_dir).whatever_context(
                                                    format!(
                                                        "Failed to create study directory: {}",
                                                        series_dir.display()
                                                    ),
                                                )?;
                                            }

                                            file_path.push(
                                                Self::sanitize_uid_component(&sop_instance_uid)?
                                                    .to_string()
                                                    + ".dcm",
                                            );

                                            log_info!("Stored {}", file_path.display());

                                            if let Err(err) = Metadata::update_study_metadata_json(
                                                &self.database,
                                                out_dir,
                                                &obj,
                                            )
                                            .await
                                            {
                                                log_error!(
                                                    "Failed to update study metadata: {}",
                                                    err
                                                );
                                            }

                                            let file_obj = obj.with_exact_meta(file_meta);

                                            file_obj.write_to_file(&file_path).whatever_context(
                                                "could not save DICOM object to file",
                                            )?;

                                            if let Some(app_handle) = &self.app_handle {
                                                app_handle
                                                    .emit(
                                                        "queue-study",
                                                        QueueUpload {
                                                            study_uid: &study_uid,
                                                        },
                                                    )
                                                    .unwrap();

                                                // Surface a disk warning if the
                                                // storage volume is running low
                                                // as studies accumulate.
                                                crate::store::disk::warn_if_low(
                                                    app_handle,
                                                    &self.config,
                                                );
                                            }

                                            let ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                                                .erased();

                                            let obj = self.create_cstore_response(
                                                message_id,
                                                &sop_class_uid,
                                                &sop_instance_uid,
                                            );

                                            let mut obj_data = Vec::new();

                                            obj.write_dataset_with_ts(&mut obj_data, &ts)
                                                .whatever_context(
                                                    "could not write response object",
                                                )?;

                                            let pdu_response = Pdu::PData {
                                                data: vec![dicom_ul::pdu::PDataValue {
                                                    presentation_context_id: data_value
                                                        .presentation_context_id,
                                                    value_type: PDataValueType::Command,
                                                    is_last: true,
                                                    data: obj_data,
                                                }],
                                            };

                                            dimse::log_scp_response(
                                                association.client_ae_title(),
                                                "C-STORE-RSP",
                                                data_value.presentation_context_id,
                                                message_id,
                                                0x0000,
                                                &[
                                                    (
                                                        "affected_sop_class_uid",
                                                        sop_class_uid.clone(),
                                                    ),
                                                    (
                                                        "affected_sop_instance_uid",
                                                        sop_instance_uid.clone(),
                                                    ),
                                                ],
                                            );

                                            association.send(&pdu_response).whatever_context(
                                                "failed to send response object to SCU",
                                            )?;
                                        }
                                    }

                                    pending_command = None;
                                    instance_buffer.clear();
                                }
                            }
                        }
                        Pdu::ReleaseRQ => {
                            buffer.clear();
                            association.send(&Pdu::ReleaseRP).unwrap_or_else(|e| {
                                log_error!(
                                    "Failed to send association release message to SCU: {}",
                                    snafu::Report::from_error(e)
                                );
                            });
                            log_info!(
                                "Released association with {}",
                                association.client_ae_title()
                            );
                            break;
                        }
                        Pdu::AbortRQ { source } => {
                            log_info!("Aborted connection from: {:?}", source);
                            break;
                        }
                        _ => {}
                    }
                }
                Err(err @ dicom_ul::association::server::Error::Receive { .. }) => {
                    if verbose {
                        log_info!("{}", Report::from_error(err));
                    } else {
                        log_info!("{}", err);
                    }
                    break;
                }
                Err(err) => {
                    log_info!("Unexpected error: {}", Report::from_error(err));
                    break;
                }
            }
        }

        if let Ok(peer_addr) = association.inner_stream().peer_addr() {
            log_info!(
                "Dropping connection with {} ({})",
                association.client_ae_title(),
                peer_addr
            );
        } else {
            log_info!("Dropping connection with {}", association.client_ae_title());
        }

        if let Some(app_handle) = &self.app_handle {
            app_handle
                .emit(
                    "log",
                    format!("Dropping connection with {}", association.client_ae_title()),
                )
                .unwrap_or_else(|e| {
                    println!("Failed to emit log event: {}", e);
                });
        }

        Ok(())
    }

    /// Validate a DICOM UID before using it as a filesystem path component.
    /// PACS implementations cannot be trusted to follow the standard, so reject
    /// any value that could escape the storage directory (path separators,
    /// `..`, or empty) rather than writing the instance to an attacker-chosen
    /// location. A legal DICOM UID is digits and dots only.
    fn sanitize_uid_component(uid: &str) -> Result<&str, Whatever> {
        let trimmed = uid.trim_end_matches('\0');
        (!trimmed.is_empty()
            && trimmed != "."
            && trimmed != ".."
            && !trimmed.contains("..")
            && !trimmed.contains('/')
            && !trimmed.contains('\\'))
        .then_some(trimmed)
        .whatever_context(format!("unsafe DICOM UID path component: {:?}", trimmed))
    }

    pub(crate) fn extract_string_tag(obj: &InMemDicomObject, tag: Tag) -> Result<String, Whatever> {
        Ok(obj
            .element(tag)
            .whatever_context(format!("missing string tag {}", tag.element()))?
            .to_str()
            .whatever_context(format!("could not retrieve {}", tag.element()))?
            .to_string())
    }

    fn extract_int_tag(obj: &InMemDicomObject, tag: Tag) -> Result<u16, Whatever> {
        obj.element(tag)
            .whatever_context(format!("missing int tag {}", tag.element()))?
            .to_int()
            .whatever_context(format!("could not retrieve {}", tag.element()))
    }

    fn extract_int_tag_optional(obj: &InMemDicomObject, tag: Tag) -> Option<u16> {
        obj.element(tag).ok()?.to_int::<u16>().ok()
    }

    /// Extract string tag from DICOM object, returning None if tag is missing or empty
    pub(crate) fn extract_string_tag_optional(
        obj: &InMemDicomObject,
        tag: dicom::core::Tag,
    ) -> Option<String> {
        match obj.element(tag) {
            Ok(element) => match element.to_str() {
                Ok(s) => {
                    let s = s.trim_end_matches('\0').to_string();
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

    fn create_cstore_response(
        &self,
        message_id: u16,
        sop_class_uid: &str,
        sop_instance_uid: &str,
    ) -> InMemDicomObject<StandardDataDictionary> {
        InMemDicomObject::command_from_element_iter([
            DataElement::new(
                tags::AFFECTED_SOP_CLASS_UID,
                VR::UI,
                dicom_value!(Str, sop_class_uid),
            ),
            DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8001])),
            DataElement::new(
                tags::MESSAGE_ID_BEING_RESPONDED_TO,
                VR::US,
                dicom_value!(U16, [message_id]),
            ),
            DataElement::new(
                tags::COMMAND_DATA_SET_TYPE,
                VR::US,
                dicom_value!(U16, [0x0101]),
            ),
            DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0x0000])),
            DataElement::new(
                tags::AFFECTED_SOP_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, sop_instance_uid),
            ),
        ])
    }

    fn create_cecho_response(&self, message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
        InMemDicomObject::command_from_element_iter([
            DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8030])),
            DataElement::new(
                tags::MESSAGE_ID_BEING_RESPONDED_TO,
                VR::US,
                dicom_value!(U16, [message_id]),
            ),
            DataElement::new(
                tags::COMMAND_DATA_SET_TYPE,
                VR::US,
                dicom_value!(U16, [0x0101]),
            ),
            DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0x0000])),
        ])
    }
}

#[cfg(test)]
mod sanitize_uid_tests {
    use super::DICOMServer;

    #[test]
    fn accepts_valid_uid_and_trims_null() {
        assert_eq!(
            DICOMServer::sanitize_uid_component("1.2.840.113619.2").unwrap(),
            "1.2.840.113619.2"
        );
        assert_eq!(
            DICOMServer::sanitize_uid_component("1.2.3\0\0").unwrap(),
            "1.2.3"
        );
    }

    #[test]
    fn rejects_traversal_and_separators() {
        for bad in ["..", ".", "../etc", "a/../b", "a/b", "a\\b", "", "\0"] {
            assert!(
                DICOMServer::sanitize_uid_component(bad).is_err(),
                "expected {:?} to be rejected",
                bad
            );
        }
    }
}
