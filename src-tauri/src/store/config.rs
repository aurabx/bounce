use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_store::{StoreExt};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub base_dir: String,
    pub api_key: String,
    // pub region: String,
    pub port: u16,
    pub host: String,
}

impl Config {



    pub fn load(app_handle: AppHandle) -> Self {

        // store: Arc<Store<tauri::Wry>>

        let store = app_handle
            .store("store.json")
            .map_err(|e| format!("Failed to load store: {}", e))
            .unwrap();

        let mut keyed_config = HashMap::new();
        let config = store.entries();
        for (key, value) in config {
            keyed_config.insert(key, value);
        }

        Config {
            api_key: match store.get("api_key") {
                None => "".to_string(),
                Some(value) => match value.as_str() {
                    Some(str_value) => str_value.parse().unwrap_or_else(|_| "".to_string()),
                    None => "".to_string(),
                },
            },

            port: match store.get("port") {
                None => 104,
                Some(value) => match value.as_str() {
                    Some(str_value) => str_value.parse().unwrap_or_else(|_| 104),
                    None => 104,
                },
            },

            host: match store.get("host") {
                None => "0.0.0.0".to_string(),
                Some(value) => match value.as_str() {
                    Some(str_value) => str_value.parse().unwrap_or_else(|_| "0.0.0.0".to_string()),
                    None => "0.0.0.0".to_string(),
                },
            },

            base_dir: match store.get("base_dir") {
                None => "./tmp/dicom_storage".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },
        }
    }

    pub fn get_base_dir(&self) -> String {
        self.base_dir.clone()
    }

    // Existing methods...

    /// Extracts the region code from the API key
    ///
    /// The API key format is expected to be "aura_REGION_bounce_TOKEN"
    /// where REGION is the region code (e.g., "au")
    ///
    /// # Examples
    ///
    /// ```
    /// let config = Config { api_key: "aura_au_bounce_ADASDASDASDASDASDA".to_string(), ... };
    /// assert_eq!(config.region_from_api_key(), Some("au".to_string()));
    /// ```
    ///
    /// Returns None if the API key doesn't match the expected format
    pub fn region_from_api_key(&self) -> Option<String> {
        let parts: Vec<&str> = self.api_key.split('_').collect();

        // Check if the API key has at least 3 parts (aura_REGION_bounce_TOKEN)
        if parts.len() >= 3 && parts[0] == "aura" {
            // Return the second part (index 1) which should be the region code
            return Some(parts[1].to_string());
        }

        None
    }

    pub fn mode_from_api_key(&self) -> Option<String> {
        let parts: Vec<&str> = self.api_key.split('_').collect();

        // Check if the API key has at least 3 parts (aura_REGION_bounce_USER_TOKEN_ENV)
        if parts.len() >= 6 && parts[0] == "aura" {
            // Return the second part (index 1) which should be the region code
            return Some(parts[5].to_string());
        }

        Some("production".to_string())
    }


    pub fn get_api_endpoint(&self) -> String {
        let region = self.region_from_api_key().unwrap();
        let mode = self.mode_from_api_key().unwrap();

        match mode.as_str() {
            "staging" => "https://staging-5em2ouy-pghszvpk65pns.au.platformsh.site".to_string(),
            "development" => "https://dev-54ta5gq-pghszvpk65pns.au.platformsh.site".to_string(),
            "local" => "https://aura.lndo.site".to_string(),
            _ => format!("https://{}.aurabox.app", region),
        }
    }

    pub fn resolve_study_path(&self, study_uid: &String) -> PathBuf {
        // Actually push the study (you’ll have to adapt to your code)
        let mut file_path = PathBuf::from(&self.get_base_dir());
        file_path.push(study_uid.trim_end_matches('\0').to_string());

        file_path
    }

    pub fn current_studies(&self) -> Value {
        let storage_dir = self.get_base_dir().clone();
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
                                if let Ok(metadata) =
                                    serde_json::from_str::<Value>(&metadata_content)
                                {
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
            ("count".to_string(), Value::from(count)),
        ]))
    }

    fn extract_field(study_info: &Value, index: &str) -> String {
        study_info
            .get(index)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or(format!("No {}", index))
    }
}
