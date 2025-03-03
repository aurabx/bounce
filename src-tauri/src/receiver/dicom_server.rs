use std::collections::HashMap;
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
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::PathBuf;
use store::config::Config;
use std::sync::{Arc};
use std::fs;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{Duration};
use transmitter::manager::{TransmissionCommand};

#[derive(Clone)]
pub struct DICOMServer {
    config: Arc<Config>,
    app_handle: AppHandle,
}


/// Represents a DICOM series in the JSON format
#[derive(Debug, Serialize, Deserialize)]
struct SeriesInfo {
    study_instance_uid: String,
    series_instance_uid: String,
    modality: Option<String>,
    series_description: Option<String>,
    body_part_examined: Option<String>,
    series_date: Option<String>,
    series_time: Option<String>,
}

/// Represents a DICOM study in the JSON format
#[derive(Debug, Serialize, Deserialize)]
struct StudyInfo {
    study_uid: String,
    study_description: Option<String>,
    institution_name: Option<String>,
    institution_address: Option<String>,
    patient_id: Option<String>,
    other_patient_ids: Option<String>,
    accession_no: Option<String>,
    patient_name: Option<String>,
    issuer_of_patient_id: Option<String>,
    patient_birth_date: Option<String>,
    patient_sex: Option<String>,
    referring_physician_name: Option<String>,
    study_date: Option<String>,
    study_time: Option<String>,
    tz_offset: Option<String>,
    series: HashMap<String, SeriesInfo>,
    images: usize,
    series_count: usize,
}


impl DICOMServer {
    pub fn new(config: Config, app_handle: AppHandle) -> Self {
        Self {
            config: Arc::new(config.clone()),
            app_handle
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let server = Arc::new(self.clone());
        let port = server.config.port;
        let out_dir = "./tmp";
        let path = PathBuf::from(&out_dir);

        fs::create_dir_all(&out_dir).unwrap_or_else(|e| {
            log_error!("Could not create output directory: {}", e);
            std::process::exit(-2);
        });

        // Bind the listener
        let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), port);
        let listener = TcpListener::bind(listen_addr)?;
        log_info!("listening on: tcp://{}", listen_addr);

        let current_path = path.clone();

        tokio::spawn({
            async move {
                for stream in listener.incoming() {
                    match stream {
                        Ok(scu_stream) => {
                            if let Err(e) = server.run_store_sync(scu_stream, &current_path).await {
                                log_error!("{}", Report::from_error(e));
                            }
                        }
                        Err(e) => {
                            log_error!("{}", Report::from_error(e));
                        }
                    }
                }
            }
        });

        Ok(())
    }



    pub async fn run_store_sync(&self, scu_stream: TcpStream, out_dir: &PathBuf) -> Result<(), Whatever> {
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
        let mut study_uid = "".to_string();
        let mut series_uid = "".to_string();
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
                                    println!("COMMAND_GROUP_LENGTH: {:?}", &obj.element(tags::COMMAND_GROUP_LENGTH).unwrap().to_str().unwrap().to_string());
                                    println!("AFFECTED_SOP_CLASS_UID: {:?}", &obj.element(tags::AFFECTED_SOP_CLASS_UID).unwrap().to_str().unwrap().to_string());
                                    println!("COMMAND_FIELD: {:?}", &obj.element(tags::COMMAND_FIELD).unwrap().to_str().unwrap().to_string());
                                    println!("MESSAGE_ID: {:?}", &obj.element(tags::MESSAGE_ID).unwrap().to_str().unwrap().to_string());
                                    println!("PRIORITY: {:?}", &obj.element(tags::PRIORITY).unwrap().to_str().unwrap().to_string());
                                    println!("COMMAND_DATA_SET_TYPE: {:?}", &obj.element(tags::COMMAND_DATA_SET_TYPE).unwrap().to_str().unwrap().to_string());
                                    println!("AFFECTED_SOP_INSTANCE_UID: {:?}", &obj.element(tags::AFFECTED_SOP_INSTANCE_UID).unwrap().to_str().unwrap().to_string());

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
                                        sop_instance_uid =
                                            Self::extract_string_tag(&obj, tags::AFFECTED_SOP_INSTANCE_UID)?;
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
                                    study_uid = Self::extract_string_tag(&obj, tags::STUDY_INSTANCE_UID)?;
                                    println!("Received StudyInstanceUID: {}", study_uid);

                                    series_uid = Self::extract_string_tag(&obj, tags::SERIES_INSTANCE_UID)?;
                                    println!("Received SeriesInstanceUID: {}", series_uid);

                                    let message = format!("Received Study: {}", study_uid);

                                    // Send message to JavaScript
                                    self.app_handle.emit("log", message).unwrap_or_else(|e| {
                                        println!("Failed to emit log event: {}", e);
                                    });

                                    let file_meta = FileMetaTableBuilder::new()
                                        .transfer_syntax(ts)
                                        .build()
                                        .whatever_context(
                                            "failed to build DICOM meta file information",
                                        )?;
                                    let file_obj = obj.with_exact_meta(file_meta);

                                    // write the files to the current directory with their SOPInstanceUID as filenames
                                    let mut file_path = out_dir.clone();

                                    file_path.push(study_uid.trim_end_matches('\0').to_string());
                                    file_path.push(series_uid.trim_end_matches('\0').to_string());

                                    let study_dir = file_path.clone();

                                    if !study_dir.exists() {
                                        fs::create_dir_all(&study_dir).whatever_context(format!(
                                            "Failed to create study directory: {}",
                                            study_dir.display()
                                        ))?;
                                    }

                                    file_path.push(
                                        sop_instance_uid.trim_end_matches('\0').to_string()
                                            + ".dcm",
                                    );
                                    file_obj
                                        .write_to_file(&file_path)
                                        .whatever_context("could not save DICOM object to file")?;

                                    log_info!("Stored {}", file_path.display());

                                    self.app_handle.emit("study-received", study_uid.clone())
                                        .unwrap_or_else(|e| {
                                            println!("Failed to emit study-received event: {}", e);
                                        });

                                    let state = self.app_handle.state::<AppState>();

                                    state.tx_manager.send_command(TransmissionCommand::ScheduleStudy {
                                        study_uid
                                    }).await;


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

        Ok(())
    }

    fn extract_string_tag(obj: &InMemDicomObject, tag: Tag) -> Result<String, Whatever> {
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
    fn extract_string_tag_optional(obj: &InMemDicomObject, tag: dicom::core::Tag) -> Option<String> {
        match obj.element(tag) {
            Ok(element) => {
                match element.to_str() {
                    Ok(s) => {
                        let s = s.trim_end_matches('\0').to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    }
                    Err(_) => None,
                }
            }
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


    /// Update the study metadata JSON file
    pub async fn update_study_metadata_json(&self, study_path: &std::path::Path, obj: &InMemDicomObject) -> Result<(), Box<dyn std::error::Error>> {
        // Extract required tags for study and series info
        let study_uid = Self::extract_string_tag(obj, tags::STUDY_INSTANCE_UID)?;
        let series_uid = Self::extract_string_tag(obj, tags::SERIES_INSTANCE_UID)?;

        // Path to the JSON metadata file
        let json_path = study_path.join("study_metadata.json");

        // Create or load existing study info
        let mut study_info = if json_path.exists() {
            // Read and parse existing JSON file
            let json_content = fs::read_to_string(&json_path)
                .map_err(|e| format!("Failed to read study metadata file: {}", e))?;

            let json_value: Value = serde_json::from_str(&json_content)
                .map_err(|e| format!("Failed to parse study metadata JSON: {}", e))?;

            // Extract the first study object from the array
            if let Some(studies) = json_value.get("studies").and_then(|s| s.as_array()) {
                if let Some(study) = studies.first() {
                    serde_json::from_value(study.clone())
                        .map_err(|e| format!("Failed to deserialize study info: {}", e))?
                } else {
                    // Create new study info if array is empty
                    self.create_new_study_info(obj, &study_uid)?
                }
            } else {
                // Create new study info if no studies array
                self.create_new_study_info(obj, &study_uid)?
            }
        } else {
            // Create new study info if no file exists
            self.create_new_study_info(obj, &study_uid)?
        };

        // Update or add series info
        let series_info = SeriesInfo {
            study_instance_uid: study_uid.clone(),
            series_instance_uid: series_uid.clone(),
            modality: Self::extract_string_tag_optional(obj, tags::MODALITY),
            series_description: Self::extract_string_tag_optional(obj, tags::SERIES_DESCRIPTION),
            body_part_examined: Self::extract_string_tag_optional(obj, tags::BODY_PART_EXAMINED),
            series_date: Self::extract_string_tag_optional(obj, tags::SERIES_DATE),
            series_time: Self::extract_string_tag_optional(obj, tags::SERIES_TIME),
        };

        // Add or update the series info
        study_info.series.insert(series_uid, series_info);

        // Update counts
        study_info.series_count = study_info.series.len();

        // Count images (one approach is to count DCM files in the study directory)
        let mut image_count = 0;
        for entry in walkdir::WalkDir::new(study_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "dcm"))
        {
            image_count += 1;
        }
        study_info.images = image_count;

        // Create the final JSON object with the studies array
        let json_obj = json!({
            "studies": [study_info]
        });

        // Write the JSON to file
        fs::write(&json_path, serde_json::to_string_pretty(&json_obj)?)
            .map_err(|e| format!("Failed to write study metadata file: {}", e))?;

        log_info!("Updated study metadata JSON: {}", json_path.display());

        Ok(())
    }

    /// Create a new StudyInfo object from a DICOM object
    fn create_new_study_info(&self, obj: &InMemDicomObject, study_uid: &str) -> Result<StudyInfo, Box<dyn std::error::Error>> {
        Ok(StudyInfo {
            study_uid: study_uid.to_string(),
            study_description: Self::extract_string_tag_optional(obj, tags::STUDY_DESCRIPTION),
            institution_name: Self::extract_string_tag_optional(obj, tags::INSTITUTION_NAME),
            institution_address: Self::extract_string_tag_optional(obj, tags::INSTITUTION_ADDRESS),
            patient_id: Self::extract_string_tag_optional(obj, tags::PATIENT_ID),
            other_patient_ids: Self::extract_string_tag_optional(obj, tags::OTHER_PATIENT_NAMES),
            accession_no: Self::extract_string_tag_optional(obj, tags::ACCESSION_NUMBER),
            patient_name: Self::extract_string_tag_optional(obj, tags::PATIENT_NAME),
            issuer_of_patient_id: Self::extract_string_tag_optional(obj, tags::ISSUER_OF_PATIENT_ID),
            patient_birth_date: Self::extract_string_tag_optional(obj, tags::PATIENT_BIRTH_DATE),
            patient_sex: Self::extract_string_tag_optional(obj, tags::PATIENT_SEX),
            referring_physician_name: Self::extract_string_tag_optional(obj, tags::REFERRING_PHYSICIAN_NAME),
            study_date: Self::extract_string_tag_optional(obj, tags::STUDY_DATE),
            study_time: Self::extract_string_tag_optional(obj, tags::STUDY_TIME),
            tz_offset: None, // TZ offset isn't directly in standard DICOM tags
            series: HashMap::new(),
            images: 0,
            series_count: 0,
        })
    }

}
