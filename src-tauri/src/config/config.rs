use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub encryption: EncryptionConfig,
    pub transmission: TransmissionConfig,
    pub dicom: DicomConfig,
    pub storage: StorageConfig,
    pub delete_after_send: (),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptionConfig {
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransmissionConfig {
    pub api_key: String,
    pub api_endpoint: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DicomConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageConfig {
    pub base_dir: String,
}

impl Config {

    pub fn load() -> Self {
        Config {
            encryption: EncryptionConfig {
                key: env::var("ENCRYPTION_KEY").unwrap_or_else(|_| "default_key".to_string()),
            },
            transmission: TransmissionConfig {
                api_key: env::var("API_KEY").unwrap_or_else(|_| "default_api_key".to_string()),
                api_endpoint: env::var("API_ENDPOINT")
                    .unwrap_or_else(|_| "https://au.aurabox.app".to_string()),
            },
            dicom: DicomConfig {
                port: env::var("DICOM_PORT")
                    .map(|p| p.parse().unwrap_or(104))
                    .unwrap_or(104),
                host: env::var("DICOM_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            },
            storage: StorageConfig {
                base_dir: env::var("STORAGE_DIR")
                    .unwrap_or_else(|_| "/tmp/dicom_storage".to_string()),
            },
            delete_after_send: (),
        }
    }

    // Setter methods for each config section
    pub fn set_encryption_key(&mut self, key: String) {
        self.encryption.key = key;
    }

    pub fn set_transmission_config(&mut self, api_key: String, api_endpoint: String) {
        self.transmission.api_key = api_key;
        self.transmission.api_endpoint = api_endpoint;
    }

    pub fn set_dicom_config(&mut self, port: u16, host: String) {
        self.dicom.port = port;
        self.dicom.host = host;
    }

    pub fn set_storage_base_dir(&mut self, base_dir: String) {
        self.storage.base_dir = base_dir;
    }
}
