use std::collections::HashMap;
use std::path::{PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;
use tokio::time::Instant;
use crate::{log_error, log_info, service};
use service::config::Config;
use service::enums::ABSTRACT_SYNTAXES;
use service::transmission::Transmission;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use dicom::core::{DataElement, VR};
use dicom::dicom_value;
use dicom::dictionary_std::tags;
use dicom::object::{FileMetaTableBuilder, InMemDicomObject, StandardDataDictionary};
use dicom::transfer_syntax::TransferSyntaxRegistry;
use dicom::encoding::TransferSyntaxIndex;
use dicom_ul::{pdu::PDataValueType, Pdu};
use snafu::{OptionExt, Report, ResultExt, Whatever};

#[derive(Clone)]
pub struct DICOMServer {
    config: Arc<Config>,
    transmission: Arc<Transmission>,
    study_timers: Arc<Mutex<HashMap<String, Instant>>>,
    study_last_received: Arc<Mutex<HashMap<String, Instant>>>,
}

impl DICOMServer {
    pub fn new(config: Config) -> Self {
        let transmission = Transmission::new(
            config.transmission.api_endpoint.clone(),
            config.transmission.api_key.clone()
        );

        Self {
            config: Arc::new(config),
            transmission: Arc::new(transmission),
            study_timers: Arc::new(Mutex::new(HashMap::new())),
            study_last_received: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {

        let port= self.config.dicom.port;
        let out_dir = "./tmp";
        let path = PathBuf::from(&out_dir);

        std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| {
            log_error!("Could not create output directory: {}", e);
            std::process::exit(-2);
        });

        let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), port);
        let listener = TcpListener::bind(listen_addr)?;
        log_info!(
            "listening on: tcp://{}",
            listen_addr
        );

        for stream in listener.incoming() {
            match stream {
                Ok(scu_stream) => {
                    if let Err(e) = self.run_store_sync(scu_stream, &path) {
                        log_error!("{}", snafu::Report::from_error(e));
                    }
                }
                Err(e) => {
                    log_error!("{}", snafu::Report::from_error(e));
                }
            }
        }

        Ok(())
    }



    pub fn run_store_sync(&self, scu_stream: TcpStream, out_dir: &PathBuf) -> Result<(), Whatever> {

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
        log_info!(
        "> Presentation contexts: {:?}",
        association.presentation_contexts()
    );

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
                                if data_value.value_type == PDataValueType::Data && !data_value.is_last
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

                                    let obj = InMemDicomObject::read_dataset_with_ts(v.as_slice(), &ts)
                                        .whatever_context("failed to read incoming DICOM command")?;
                                    let command_field = obj
                                        .element(tags::COMMAND_FIELD)
                                        .whatever_context("Missing Command Field")?
                                        .uint16()
                                        .whatever_context("Command Field is not an integer")?;

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
                                        msgid = obj
                                            .element(tags::MESSAGE_ID)
                                            .whatever_context("Missing Message ID")?
                                            .to_int()
                                            .whatever_context("Message ID is not an integer")?;
                                        sop_class_uid = obj
                                            .element(tags::AFFECTED_SOP_CLASS_UID)
                                            .whatever_context("missing Affected SOP Class UID")?
                                            .to_str()
                                            .whatever_context(
                                                "could not retrieve Affected SOP Class UID",
                                            )?
                                            .to_string();
                                        sop_instance_uid = obj
                                            .element(tags::AFFECTED_SOP_INSTANCE_UID)
                                            .whatever_context("missing Affected SOP Instance UID")?
                                            .to_str()
                                            .whatever_context(
                                                "could not retrieve Affected SOP Instance UID",
                                            )?
                                            .to_string();
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
                                    let file_meta = FileMetaTableBuilder::new()
                                        .media_storage_sop_class_uid(
                                            obj.element(tags::SOP_CLASS_UID)
                                                .whatever_context("missing SOP Class UID")?
                                                .to_str()
                                                .whatever_context("could not retrieve SOP Class UID")?,
                                        )
                                        .media_storage_sop_instance_uid(
                                            obj.element(tags::SOP_INSTANCE_UID)
                                                .whatever_context("missing SOP Instance UID")?
                                                .to_str()
                                                .whatever_context("missing SOP Instance UID")?,
                                        )
                                        .transfer_syntax(ts)
                                        .build()
                                        .whatever_context(
                                            "failed to build DICOM meta file information",
                                        )?;
                                    let file_obj = obj.with_exact_meta(file_meta);

                                    // write the files to the current directory with their SOPInstanceUID as filenames
                                    let mut file_path = out_dir.clone();
                                    file_path.push(
                                        sop_instance_uid.trim_end_matches('\0').to_string() + ".dcm",
                                    );
                                    file_obj
                                        .write_to_file(&file_path)
                                        .whatever_context("could not save DICOM object to file")?;
                                    log_info!("Stored {}", file_path.display());

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
                                            presentation_context_id: data_value.presentation_context_id,
                                            value_type: PDataValueType::Command,
                                            is_last: true,
                                            data: obj_data,
                                        }],
                                    };
                                    association
                                        .send(&pdu_response)
                                        .whatever_context("failed to send response object to SCU")?;
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

        Ok(())
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


    //
    // async fn handle_store(&self, dataset: DataSet) -> Result<(), Box<dyn std::error::Error>> {
    //     let study_id = dataset.get_string("StudyInstanceUID")
    //         .unwrap_or_else(|_| "unknown_study".to_string());
    //     let series_id = dataset.get_string("SeriesInstanceUID")
    //         .unwrap_or_else(|_| "unknown_series".to_string());
    //     let sop_instance = dataset.get_string("SOPInstanceUID")
    //         .unwrap_or_else(|_| "unknown_sop".to_string());
    //
    //     let study_path = Path::new(&self.config.storage.base_dir)
    //         .join(&study_id)
    //         .join(&series_id);
    //
    //     std::fs::create_dir_all(&study_path)?;
    //
    //     let file_path = study_path.join(format!("{}.dcm", sop_instance));
    //
    //     // Save DICOM file
    //     // Actual implementation would use dicom crate's serialization
    //     std::fs::write(&file_path, b"placeholder_dicom_data")?;
    //
    //     // Update last received time for the study
    //     let mut last_received = self.study_last_received.lock().await;
    //     last_received.insert(study_id.clone(), Instant::now());
    //
    //     // Schedule study push
    //     self.schedule_study_push(study_id.clone()).await?;
    //
    //     Ok(())
    // }

    async fn schedule_study_push(&self, study_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let timeout = Duration::from_secs(60);
        //let transmission = Arc::clone(&self.transmission);
        //let config_clone = Arc::clone(&self.config);
        let study_last_received = Arc::clone(&self.study_last_received);

        tokio::spawn(async move {
            time::sleep(timeout).await;

            let last_received = {
                study_last_received.lock().await
                    .get(&study_id).cloned()
            };

            if let Some(last_time) = last_received {
                let time_since_last = last_time.elapsed();
                log_info!("Last file for study {} was {} seconds ago",
                    study_id, time_since_last.as_secs_f64()
                );
            }

            // let study_path = Path::new(&config_clone.storage.base_dir).join(study_id);

            // if let Err(e) = transmission.send_archive(
            //     &study_path,
            //     config_clone.delete_after_send.unwrap_or(false)
            // ).await {
            //     log_error!("Failed to push study: {}", e);
            // }
        });

        Ok(())
    }
}