use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::{json, Value};
use tauri_plugin_store::Store;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub base_dir: String,
    pub api_key: String,
    pub region: String,
    pub port: u16,
    pub host: String,
    pub mode: String,
}

impl Config {
    pub fn load(store: Arc<Store<tauri::Wry>>) -> Self {
        let mut keyed_config = HashMap::new();
        let config = store.entries();
        for (key, value) in config {
            keyed_config.insert(key, value);
        }

        // let api_key = store
        //     .get("api_key")
        //     .expect("No API key found in store")
        //     .to_string();

        // println!("api_key in command: {}", api_key);
        // println!(
        //     "store.entries(): {}",
        //     serde_json::to_string(&keyed_config).unwrap().to_string()
        // );

        Config {
            api_key: match store.get("api_key") {
                None => "API key not found in store".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            region: match store.get("region") {
                None => "Region not found in store".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            port: match store.get("port") {
                None => 104,
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            host: match store.get("host") {
                None => "0.0.0.0".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            mode: match store.get("mode") {
                None => "production".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            base_dir: match store.get("base_dir") {
                None => "./tmp/dicom_storage".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

        }
    }

    pub fn get_api_endpoint(&self) -> String {
        let region = self.region.clone();
        let region = region.as_str();

        match self.mode.as_str() {
            "staging" => "https://staging.aurabox.app".to_string(),
            "development" => "https://dev-54ta5gq-pghszvpk65pns.au.platformsh.site".to_string(),
            "local" => "https://aura.lndo.site".to_string(),
            _ => format!("https://{}.aurabox.app", region),
        }
    }

    pub fn current_studies(&self) -> Value {
        let storage_dir = self.base_dir.clone();
        let storage_dir = storage_dir.as_str();

        // Create a path to the storage directory
        let path = std::path::Path::new(storage_dir);

        // Initialize an empty array to store our results
        let mut studies = Value::Array(Vec::new());

        // Check if the directory exists
        if path.exists() && path.is_dir() {
            // Read the directory entries
            if let Ok(entries) = std::fs::read_dir(path) {
                // Process each entry in the directory
                for entry in entries.filter_map(Result::ok) {
                    let entry_path = entry.path();

                    // Check if this is a directory
                    if entry_path.is_dir() {
                        // Look for metadata.json in this directory
                        let metadata_path = entry_path.join("metadata.json");

                        if metadata_path.exists() && metadata_path.is_file() {
                            // Read and parse the metadata file
                            if let Ok(metadata_content) = std::fs::read_to_string(&metadata_path) {
                                if let Ok(metadata) = serde_json::from_str::<Value>(&metadata_content) {
                                    // Extract information from the metadata
                                    if let Some(studies_data) = metadata.get("studies") {
                                        // Iterate through each study in the metadata
                                        if let Some(studies_obj) = studies_data.as_object() {
                                            for (_, study_info) in studies_obj {

                                                // Create a study object with extracted information
                                                let study = json!({
                                                    "study_uid": Self::extract_field(study_info, "study_uid"),
                                                    "study_description": Self::extract_field(study_info, "study_description"),
                                                    "study_date": Self::extract_field(study_info, "study_date"),
                                                    "study_time": Self::extract_field(study_info, "study_time"),
                                                    "path": entry_path.to_string_lossy()
                                                });

                                                // Add the study to our array
                                                studies.as_array_mut().unwrap().push(study);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let count = studies.as_array().unwrap().len();

        // Return the array as a JSON value
        Value::Object(serde_json::Map::from_iter([
            ("studies".to_string(), studies),
            ("count".to_string(), Value::from(count))
        ]))
    }

    fn extract_field(study_info: &Value, index: &str) -> String {
        study_info.get(index)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or(format!("No {}", index))

    }
}
