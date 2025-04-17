use crate::receiver::metadata::Metadata;
use crate::{log_error, log_info, receiver, store, transmitter, AppState};
use dicom::core::{DataElement, Tag, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::encoding::TransferSyntaxIndex;
use dicom::object::{FileMetaTableBuilder, InMemDicomObject, StandardDataDictionary};
use dicom::transfer_syntax::TransferSyntaxRegistry;
use dicom_ul::{pdu::PDataValueType, Pdu};
use receiver::enums::ABSTRACT_SYNTAXES;
use snafu::{OptionExt, Report, ResultExt, Whatever};
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use store::config::Config;
use tauri::{AppHandle, Emitter, Manager};
use transmitter::manager::TransmissionCommand;

#[derive(Clone)]
pub struct DICOMServer {
    config: Arc<Config>,
    app_handle: AppHandle,
}

impl DICOMServer {
    pub fn new(config: Config, app_handle: AppHandle) -> Self {
        Self {
            config: Arc::new(config.clone()),
            app_handle,
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
        let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), port);
        let listener = TcpListener::bind(listen_addr)?;
        log_info!("listening on: tcp://{}", listen_addr);

        // Convert to tokio listener - this lets us use accept_async
        let listener = tokio::net::TcpListener::from_std(listener)?;

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
        out_dir: &PathBuf,
    ) -> Result<(), Whatever> {
        let verbose = true;
        let strict = false;
        let calling_ae_title = "STORE-SCP";
        let uncompressed_only = false;
        let promiscuous = true;
        let max_pdu_length = 16384;

        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        let mut instance_buffer: Vec<u8> = Vec::with_capacity(1024 * 1024);
        let mut msgid = 1;
        let mut sop_class_uid = "".to_string();
        let mut sop_instance_uid = "".to_string();

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

        let mut association = options
            .establish(scu_stream)
            .whatever_context("could not establish association")?;

        log_info!("New association from {}", association.client_ae_title());

        // Send message to JavaScript
        self.app_handle.emit("log", format!("New association from {}", association.client_ae_title())).unwrap_or_else(|e| {
            println!("Failed to emit log event: {}", e);
        });


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

                                    println!("Tags: {:?}", &obj.tags().collect::<Vec<_>>());
                                    println!(
                                        "COMMAND_GROUP_LENGTH: {:?}",
                                        &obj.element(tags::COMMAND_GROUP_LENGTH)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );
                                    println!(
                                        "AFFECTED_SOP_CLASS_UID: {:?}",
                                        &obj.element(tags::AFFECTED_SOP_CLASS_UID)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );
                                    println!(
                                        "COMMAND_FIELD: {:?}",
                                        &obj.element(tags::COMMAND_FIELD)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );
                                    println!(
                                        "MESSAGE_ID: {:?}",
                                        &obj.element(tags::MESSAGE_ID)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );
                                    println!(
                                        "PRIORITY: {:?}",
                                        &obj.element(tags::PRIORITY)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );
                                    println!(
                                        "COMMAND_DATA_SET_TYPE: {:?}",
                                        &obj.element(tags::COMMAND_DATA_SET_TYPE)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );
                                    println!(
                                        "AFFECTED_SOP_INSTANCE_UID: {:?}",
                                        &obj.element(tags::AFFECTED_SOP_INSTANCE_UID)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_string()
                                    );

                                    if command_field == 0x0030 {
                                        // Handle C-ECHO-RQ
                                        let cecho_response = self.create_cecho_response(msgid);
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
                                        association.send(&pdu_response).whatever_context(
                                            "failed to send C-ECHO response object to SCU",
                                        )?;
                                    } else {
                                        msgid = Self::extract_int_tag(&obj, tags::MESSAGE_ID)?;
                                        sop_class_uid = Self::extract_string_tag(
                                            &obj,
                                            tags::AFFECTED_SOP_CLASS_UID,
                                        )?;
                                        sop_instance_uid = Self::extract_string_tag(
                                            &obj,
                                            tags::AFFECTED_SOP_INSTANCE_UID,
                                        )?;
                                    }
                                    instance_buffer.clear();
                                } else if data_value.value_type == PDataValueType::Data
                                    && data_value.is_last
                                {
                                    instance_buffer.append(&mut data_value.data);

                                    let presentation_context = association
                                        .presentation_contexts()
                                        .iter()
                                        .find(|pc| pc.id == data_value.presentation_context_id)
                                        .whatever_context("missing presentation context")?;
                                    let ts = &presentation_context.transfer_syntax;

                                    let obj = InMemDicomObject::read_dataset_with_ts(
                                        instance_buffer.as_slice(),
                                        TransferSyntaxRegistry.get(ts).unwrap(),
                                    )
                                    .whatever_context("failed to read DICOM data object")?;

                                    // Extract StudyInstanceUID
                                    let study_uid =
                                        Self::extract_string_tag(&obj, tags::STUDY_INSTANCE_UID)?;
                                    println!("Received StudyInstanceUID: {}", study_uid);

                                    let series_uid =
                                        Self::extract_string_tag(&obj, tags::SERIES_INSTANCE_UID)?;
                                    println!("Received SeriesInstanceUID: {}", series_uid);

                                    // let message = format!("Received Study: {}", study_uid);
                                    
                                    let file_meta = FileMetaTableBuilder::new()
                                        .transfer_syntax(ts)
                                        .build()
                                        .whatever_context(
                                            "failed to build DICOM meta file information",
                                        )?;

                                    // write the files to the current directory with their SOPInstanceUID as filenames
                                    let mut file_path = out_dir.clone();

                                    file_path.push(study_uid.trim_end_matches('\0').to_string());
                                    let study_dir = file_path.clone();

                                    file_path.push(series_uid.trim_end_matches('\0').to_string());
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
                                        sop_instance_uid.trim_end_matches('\0').to_string()
                                            + ".dcm",
                                    );

                                    log_info!("Stored {}", file_path.display());

                                    if let Err(err) = Metadata::update_study_metadata_json(
                                        study_dir.as_path(),
                                        &obj,
                                    )
                                    .await
                                    {
                                        log_error!("Failed to update study metadata: {}", err);
                                    }

                                    let file_obj = obj.with_exact_meta(file_meta);

                                    file_obj
                                        .write_to_file(&file_path)
                                        .whatever_context("could not save DICOM object to file")?;

                                    let state = self.app_handle.state::<AppState>();
                                    
                                    state
                                        .tx_manager
                                        .send_command(TransmissionCommand::ScheduleStudy {
                                            study_uid,
                                        })
                                        .await;

                                    // send C-STORE-RSP object
                                    // commands are always in implict VR LE
                                    let ts =
                                        dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                                            .erased();

                                    let obj = self.create_cstore_response(
                                        msgid,
                                        &sop_class_uid,
                                        &sop_instance_uid,
                                    );

                                    let mut obj_data = Vec::new();

                                    obj.write_dataset_with_ts(&mut obj_data, &ts)
                                        .whatever_context("could not write response object")?;

                                    let pdu_response = Pdu::PData {
                                        data: vec![dicom_ul::pdu::PDataValue {
                                            presentation_context_id: data_value
                                                .presentation_context_id,
                                            value_type: PDataValueType::Command,
                                            is_last: true,
                                            data: obj_data,
                                        }],
                                    };

                                    association.send(&pdu_response).whatever_context(
                                        "failed to send response object to SCU",
                                    )?;
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

        self.app_handle.emit("log", format!("Dropping connection with {}", association.client_ae_title())).unwrap_or_else(|e| {
            println!("Failed to emit log event: {}", e);
        });

        Ok(())
    }

    pub(crate) fn extract_string_tag(obj: &InMemDicomObject, tag: Tag) -> Result<String, Whatever> {
        Ok(obj
            .element(tag)
            .whatever_context(format!("missing string tag {}", tag.element().to_string()))?
            .to_str()
            .whatever_context(format!("could not retrieve {}", tag.element().to_string()))?
            .to_string())
    }

    fn extract_int_tag(obj: &InMemDicomObject, tag: Tag) -> Result<u16, Whatever> {
        Ok(obj
            .element(tag)
            .whatever_context(format!("missing int tag {}", tag.element().to_string()))?
            .to_int()
            .whatever_context(format!("could not retrieve {}", tag.element().to_string()))?)
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
