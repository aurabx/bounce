use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub base_dir: String,
    pub api_key: String,
    pub port: u16,
    pub ip_address: String,
    pub ae_title: String,
    pub delete_after_success: String,
    pub send_logs: String,
}

impl Config {
    pub fn load(app_handle: AppHandle) -> Self {
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
                None => 9090,
                Some(value) => match value.as_str() {
                    Some(str_value) => str_value.parse().unwrap_or(9090),
                    None => 9090,
                },
            },

            ip_address: match store.get("ip_address") {
                None => "0.0.0.0".to_string(),
                Some(value) => match value.as_str() {
                    Some(str_value) => {
                        if str_value.trim().is_empty() {
                            "0.0.0.0".to_string()
                        } else {
                            str_value.parse().unwrap_or_else(|_| "0.0.0.0".to_string())
                        }
                    }
                    None => "0.0.0.0".to_string(),
                },
            },

            ae_title: match store.get("ae_title") {
                None => "BOUNCE".to_string(),
                Some(value) => match value.as_str() {
                    Some(str_value) => str_value.parse().unwrap_or_else(|_| "BOUNCE".to_string()),
                    None => "BOUNCE".to_string(),
                },
            },

            base_dir: match store.get("base_dir") {
                None => "./tmp/dicom_storage".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            delete_after_success: match store.get("delete_after_success") {
                None => "no".to_string(),
                Some(value) => value.as_str().unwrap().parse().unwrap(),
            },

            send_logs: match store.get("send_logs") {
                None => "yes".to_string(),
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
            "dev" => "https://dev-54ta5gq-pghszvpk65pns.au.platformsh.site".to_string(),
            "local" => "https://aura.lndo.site".to_string(),
            _ => format!("https://{}.aurabox.app", region),
        }
    }

    pub fn resolve_study_path(&self, study_uid: &str) -> PathBuf {
        // Actually push the study (you’ll have to adapt to your code)
        let mut file_path = PathBuf::from(&self.get_base_dir());
        file_path.push(study_uid.trim_end_matches('\0'));
        file_path
    }

    pub fn resolve_metadata_path(&self, study_uid: &str) -> PathBuf {
        // Actually push the study (you’ll have to adapt to your code)
        let mut file_path = PathBuf::from(&self.get_base_dir());
        file_path.push(study_uid.trim_end_matches('\0').to_string() + ".json");
        file_path
    }
}
