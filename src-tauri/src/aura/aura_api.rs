use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Error;
use reqwest::{Client, Response};
use serde_json::{json, Value};
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

        println!(
            "generate_signature (url: {})",
            url
        );

        let response = self.client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .send()
            .await?;

        Self::handle_response(response).await
    }


    pub async fn upload_start(
        &self,
        study_uid: String,
        signature: String,
        upload_id: String
    ) -> anyhow::Result<Value> {
        println!(
            "upload_start (study_uid: {}), (signature: {}), (signature: {})",
            study_uid, signature, upload_id
        );


        let out_dir = PathBuf::from(&self.config.base_dir);
        let mut file_path = out_dir.clone();
        file_path.push(study_uid.trim_end_matches('\0').to_string());

        let study_path = file_path.as_path();
        let json_path = study_path.join("metadata.json");

        let json_content = fs::read_to_string(&json_path)?;

        let json_value: Value = serde_json::from_str::<Value>(&json_content)?;

        let url = format!("{}/api/bounce/upload/start", &self.config.get_api_endpoint());

        let response = self.client
            .post(&url)
            .json(&json!({
                "studies" : json_value.get("studies").unwrap(),
                "mode" : "supplier",
                "signature" : signature,
                "upload_id" : upload_id,
            }))
            .header("Content-Type", "application/json")
            .header("Accepts", "application/json")
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .send()
            .await?;

        Self::handle_response(response).await
    }

    async fn handle_response(response: Response) -> Result<Value, Error> {
        // Check HTTP status code first
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;

            // Try to parse error as JSON if possible
            let error_detail = match serde_json::from_str::<Value>(&body) {
                Ok(json_error) => json_error,
                Err(_) => json!({"raw_error": body}),
            };

            println!(
                "API request failed with status {}: {:?}",
                status,
                error_detail
            );

            return Err(anyhow::anyhow!(
                "API request failed with status {}: {:?}",
                status,
                error_detail
            ));
        }


        let body = response.text().await?;
        let result: Value = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!("Failed to parse API response: {}. Raw response: {}", e, body)
        })?;

        Ok(result)
    }
}