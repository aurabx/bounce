use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use reqwest::Client;
use serde_json::{json, Value};
use tauri::AppHandle;
use crate::store::config::Config;

#[derive(Clone, Debug)]
pub struct AuraApi {
    client: Client,
    config: Arc<Config>
}

impl AuraApi {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config: Arc::new(config.clone())
        }
    }


    pub async fn generate_signature(&self) -> anyhow::Result<Value> {
        let url = format!("{}/api/bounce/signature", &self.config.get_api_endpoint());

        let response = self.client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .send()
            .await?;


        let body = response.text().await?;
        let result: Value = serde_json::from_str(&body)?;

        Ok(result)
    }



    pub async fn upload_start(
        &self, 
        study_uid: String, 
        signature: String, 
        upload_id: String
    ) -> anyhow::Result<Value> {
        let out_dir = PathBuf::from("./tmp");
        let mut file_path = out_dir.clone();
        file_path.push(study_uid.trim_end_matches('\0').to_string());

        let study_path = file_path.as_path();
        let json_path = study_path.join("study_metadata.json");

        let json_content = fs::read_to_string(&json_path)
            .map_err(|e| format!("Failed to read study metadata file: {}", e))?;

        let json_value: Value = serde_json::from_str(&json_content)
            .map_err(|e| format!("Failed to parse study metadata JSON: {}", e))?;

        let url = format!("{}/api/upload/signature", &self.config.get_api_endpoint());

        let response = self.client
            .post(&url)
            .json(&json!({
                "studies" : json_value.get("studies").unwrap(),
                "mode" : "supplier",
                "signature" : signature,
                "upload_id" : upload_id,
            }))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .send()
            .await?;


        let body = response.text().await?;
        let result: Value = serde_json::from_str(&body)?;

        Ok(result)
    }
}