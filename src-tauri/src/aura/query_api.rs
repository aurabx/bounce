//! Testable HTTP client for C-FIND query-related Aurabox endpoints.
//!
//! [`QueryApiClient`] encapsulates the HTTP request/response logic for the
//! query polling endpoints without requiring a Tauri `AppHandle`. This allows
//! the methods to be tested against a mock HTTP server.
//!
//! The main [`super::aura_api::AuraApi`] delegates to this client, resolving
//! the `base_url` and `api_key` from the Tauri config store.

use crate::query::models::{
    CfindResult, PendingQueriesResponse, QueryFailedPayload, QueryResultsPayload,
    ServicesResponse,
};
use reqwest::Client;
use serde_json::Value;

/// HTTP client for query-related Aurabox API endpoints.
///
/// Unlike [`super::aura_api::AuraApi`], this struct does not depend on Tauri
/// and can be constructed directly with a base URL and API key for testing.
#[derive(Clone, Debug)]
pub struct QueryApiClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl QueryApiClient {
    /// Create a new client pointing at the given Aurabox base URL.
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            api_key,
        }
    }

    /// Fetch configured DICOM services (remote PACS) from Aurabox.
    ///
    /// Calls `GET {base_url}/api/bounce/queries/services`.
    pub async fn fetch_services(&self) -> anyhow::Result<ServicesResponse> {
        let url = format!("{}/api/bounce/queries/services", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to fetch services: HTTP {} - {}",
                status,
                body,
            ));
        }

        let body = response.text().await?;
        let parsed: ServicesResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse services response: {}. Raw: {}",
                e,
                body,
            )
        })?;

        Ok(parsed)
    }

    /// Fetch pending C-FIND queries from Aurabox for this gateway.
    ///
    /// Calls `GET {base_url}/api/bounce/queries/pending`.
    pub async fn fetch_pending_queries(&self) -> anyhow::Result<PendingQueriesResponse> {
        let url = format!("{}/api/bounce/queries/pending", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to fetch pending queries: HTTP {} - {}",
                status,
                body,
            ));
        }

        let body = response.text().await?;
        let parsed: PendingQueriesResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse pending queries response: {}. Raw: {}",
                e,
                body,
            )
        })?;

        Ok(parsed)
    }

    /// Post C-FIND results back to Aurabox.
    ///
    /// Calls `POST {base_url}/api/bounce/queries/{id}/results`.
    pub async fn post_query_results(
        &self,
        query_id: &str,
        results: Vec<CfindResult>,
    ) -> anyhow::Result<Value> {
        let url = format!(
            "{}/api/bounce/queries/{}/results",
            self.base_url, query_id,
        );

        let payload = QueryResultsPayload { results };

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await?;

        Self::handle_response(response).await
    }

    /// Report a C-FIND query failure to Aurabox.
    ///
    /// Calls `POST {base_url}/api/bounce/queries/{id}/failed`.
    pub async fn post_query_failed(
        &self,
        query_id: &str,
        error: String,
    ) -> anyhow::Result<Value> {
        let url = format!(
            "{}/api/bounce/queries/{}/failed",
            self.base_url, query_id,
        );

        let payload = QueryFailedPayload { error };

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await?;

        Self::handle_response(response).await
    }

    async fn handle_response(response: reqwest::Response) -> anyhow::Result<Value> {
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;

            let error_detail = match serde_json::from_str::<Value>(&body) {
                Ok(json_error) => json_error,
                Err(_) => serde_json::json!({"raw_error": body}),
            };

            return Err(anyhow::anyhow!(
                "API request failed with status {}: {:?}",
                status,
                error_detail
            ));
        }

        let body = response.text().await?;
        let result: Value = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse API response: {}. Raw response: {}",
                e,
                body
            )
        })?;

        Ok(result)
    }
}
