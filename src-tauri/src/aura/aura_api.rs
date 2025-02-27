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
}