use crate::receiver::dicom_server::DICOMServer;
use crate::{log_error, log_info};
use dicom::object::InMemDicomObject;
use dicom_dictionary_std::tags;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Metadata {}

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
    series: HashMap<String, self::SeriesInfo>,
    images: usize,
    series_count: usize,
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

impl Metadata {
    /// Update the study metadata JSON file with study_uid as the key
    pub async fn update_study_metadata_json(
        out_path: &Path,
        obj: &InMemDicomObject,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Extract required tags for study and series info
        let study_uid = DICOMServer::extract_string_tag(obj, tags::STUDY_INSTANCE_UID)?;
        let series_uid = DICOMServer::extract_string_tag(obj, tags::SERIES_INSTANCE_UID)?;

        // Path to the JSON metadata file
        let study_path = out_path.join(study_uid.clone());
        let json_path = out_path.join(study_uid.clone() + ".json");

        // Create new HashMap for studies if no file exists or load existing
        let mut studies_map: HashMap<String, StudyInfo> = if json_path.exists() {
            // Read and parse existing JSON file
            let json_content = fs::read_to_string(&json_path)
                .map_err(|e| format!("Failed to read study metadata file: {}", e))?;

            let json_value: Value = serde_json::from_str(&json_content)
                .map_err(|e| format!("Failed to parse study metadata JSON: {}", e))?;

            // Extract the studies object
            if let Some(studies) = json_value.get("studies").and_then(|s| s.as_object()) {
                // Convert to our HashMap
                let mut map = HashMap::new();
                for (study_id, study_value) in studies {
                    match serde_json::from_value::<StudyInfo>(study_value.clone()) {
                        Ok(study_info) => {
                            map.insert(study_id.clone(), study_info);
                        }
                        Err(e) => {
                            log_error!("Failed to deserialize study info for {}: {}", study_id, e);
                            // Continue with other studies
                        }
                    }
                }
                map
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        // Get or create the study info
        let study_info = studies_map.entry(study_uid.clone()).or_insert_with(|| {
            // Create new study info
            Self::create_new_study_info(obj, &study_uid).unwrap_or_else(|e| {
                log_error!("Error creating study info: {}", e);
                // Return a default study info with only the UID
                StudyInfo {
                    study_uid: study_uid.clone(),
                    study_description: None,
                    institution_name: None,
                    institution_address: None,
                    patient_id: None,
                    other_patient_ids: None,
                    accession_no: None,
                    patient_name: None,
                    issuer_of_patient_id: None,
                    patient_birth_date: None,
                    patient_sex: None,
                    referring_physician_name: None,
                    study_date: None,
                    study_time: None,
                    tz_offset: None,
                    series: HashMap::new(),
                    images: 0,
                    series_count: 0,
                }
            })
        });

        // Update or add series info
        let series_info = SeriesInfo {
            study_instance_uid: study_uid.clone(),
            series_instance_uid: series_uid.clone(),
            modality: DICOMServer::extract_string_tag_optional(obj, tags::MODALITY),
            series_description: DICOMServer::extract_string_tag_optional(
                obj,
                tags::SERIES_DESCRIPTION,
            ),
            body_part_examined: DICOMServer::extract_string_tag_optional(
                obj,
                tags::BODY_PART_EXAMINED,
            ),
            series_date: DICOMServer::extract_string_tag_optional(obj, tags::SERIES_DATE),
            series_time: DICOMServer::extract_string_tag_optional(obj, tags::SERIES_TIME),
        };

        // Add or update the series info
        study_info.series.insert(series_uid, series_info);

        // Update counts
        study_info.series_count = study_info.series.len();

        // Count images (one approach is to count DCM files in the study directory)
        let mut image_count = 0;
        for _entry in walkdir::WalkDir::new(study_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "dcm"))
        {
            image_count += 1;
        }
        study_info.images = image_count;

        // Create the final JSON object with the studies map
        let json_obj = json!({
            "studies": studies_map,
            "status": "IN-PROGRESS",
        });


        // Write the JSON to file
        fs::write(&json_path, serde_json::to_string_pretty(&json_obj)?)
            .map_err(|e| format!("Failed to write study metadata file: {}", e))?;

        log_info!("Updated study metadata JSON: {}", json_path.display());

        Ok(())
    }
    /// Create a new StudyInfo object from a DICOM object
    fn create_new_study_info(
        obj: &InMemDicomObject,
        study_uid: &str,
    ) -> Result<StudyInfo, Box<dyn std::error::Error>> {
        Ok(StudyInfo {
            study_uid: study_uid.to_string(),
            study_description: DICOMServer::extract_string_tag_optional(
                obj,
                tags::STUDY_DESCRIPTION,
            ),
            institution_name: DICOMServer::extract_string_tag_optional(obj, tags::INSTITUTION_NAME),
            institution_address: DICOMServer::extract_string_tag_optional(
                obj,
                tags::INSTITUTION_ADDRESS,
            ),
            patient_id: DICOMServer::extract_string_tag_optional(obj, tags::PATIENT_ID),
            other_patient_ids: DICOMServer::extract_string_tag_optional(
                obj,
                tags::OTHER_PATIENT_NAMES,
            ),
            accession_no: DICOMServer::extract_string_tag_optional(obj, tags::ACCESSION_NUMBER),
            patient_name: DICOMServer::extract_string_tag_optional(obj, tags::PATIENT_NAME),
            issuer_of_patient_id: DICOMServer::extract_string_tag_optional(
                obj,
                tags::ISSUER_OF_PATIENT_ID,
            ),
            patient_birth_date: DICOMServer::extract_string_tag_optional(
                obj,
                tags::PATIENT_BIRTH_DATE,
            ),
            patient_sex: DICOMServer::extract_string_tag_optional(obj, tags::PATIENT_SEX),
            referring_physician_name: DICOMServer::extract_string_tag_optional(
                obj,
                tags::REFERRING_PHYSICIAN_NAME,
            ),
            study_date: DICOMServer::extract_string_tag_optional(obj, tags::STUDY_DATE),
            study_time: DICOMServer::extract_string_tag_optional(obj, tags::STUDY_TIME),
            tz_offset: None, // TZ offset isn't directly in standard DICOM tags
            series: HashMap::new(),
            images: 0,
            series_count: 0,
        })
    }


    /// Update the study metadata JSON file with study_uid as the key
    pub async fn update_study_metadata_status(
        out_path: &Path,
        study_uid: String,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Path to the JSON metadata file
        // let study_path = out_path.join(study_uid.clone());

        let json_path = out_path.join(study_uid.clone() + ".json");

        // Create new HashMap for studies if no file exists or load existing
        let studies_map: HashMap<String, StudyInfo> = if json_path.exists() {
            // Read and parse existing JSON file
            let json_content = fs::read_to_string(&json_path)
                .map_err(|e| format!("Failed to read study metadata file: {}", e))?;

            let json_value: Value = serde_json::from_str(&json_content)
                .map_err(|e| format!("Failed to parse study metadata JSON: {}", e))?;

            // Extract the studies object
            if let Some(studies) = json_value.get("studies").and_then(|s| s.as_object()) {
                // Convert to our HashMap
                let mut map = HashMap::new();
                for (study_id, study_value) in studies {
                    match serde_json::from_value::<StudyInfo>(study_value.clone()) {
                        Ok(study_info) => {
                            map.insert(study_id.clone(), study_info);
                        }
                        Err(e) => {
                            log_error!("Failed to deserialize study info for {}: {}", study_id, e);
                            // Continue with other studies
                        }
                    }
                }
                map
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        // Create the final JSON object with the studies map
        let json_obj = json!({
            "studies": studies_map,
            "status": status,
        });

        // Write the JSON to file
        fs::write(&json_path, serde_json::to_string_pretty(&json_obj)?)
            .map_err(|e| format!("Failed to write study metadata file: {}", e))?;

        log_info!("Updated study metadata status JSON: {}", json_path.display());

        Ok(())
    }
}
