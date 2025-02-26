use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tauri_plugin_store::Store;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub base_dir: String,
    pub api_key: String,
    pub region: String,
    pub port: u16,
    pub host: String,
}

impl Config {
    pub fn load(store: Arc<Store<tauri::Wry>>) -> Self {
        let mut keyed_config = HashMap::new();
        let config = store.entries();
        for (key, value) in config {
            keyed_config.insert(key, value);
        }

        let api_key = store
            .get("api_key")
            .expect("No API key found in store")
            .to_string();

        println!("api_key in command: {}", api_key);
        println!(
            "store.entries(): {}",
            serde_json::to_string(&keyed_config).unwrap().to_string()
        );

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

            base_dir: env::var("STORAGE_DIR").unwrap_or_else(|_| "/tmp/dicom_storage".to_string()),
        }
    }

    pub fn get_api_endpoint(&self) -> String {
        "https://webhook.site/7fcc2fb1-a457-4c2f-a9c4-40f5863f7a95".to_string()
        // self.region.clone() + ".aurabox.app"
    }
}
