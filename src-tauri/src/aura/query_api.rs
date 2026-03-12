//! Testable HTTP client for C-FIND query-related Aurabox endpoints.
//!
//! [`QueryApiClient`] encapsulates the HTTP request/response logic for the
//! query polling endpoints without requiring a Tauri `AppHandle`. This allows
//! the methods to be tested against a mock HTTP server.
//!
//! The main [`super::aura_api::AuraApi`] delegates to this client, resolving
//! the `base_url` and `api_key` from the Tauri config store.

use crate::query::models::{
    CfindResult, FindStudiesResponse, FindStudyRequest, PendingQueriesResponse,
    PendingRetrievesResponse, QueryFailedPayload, QueryResultsPayload, RetrieveFailedPayload,
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

    // -------------------------------------------------------------------
    // C-MOVE retrieve endpoints
    // -------------------------------------------------------------------

    /// Fetch pending C-MOVE retrieve requests from Aurabox for this gateway.
    ///
    /// Calls `GET {base_url}/api/bounce/retrieves/pending`.
    pub async fn fetch_pending_retrieves(&self) -> anyhow::Result<PendingRetrievesResponse> {
        let url = format!("{}/api/bounce/retrieves/pending", self.base_url);

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
                "Failed to fetch pending retrieves: HTTP {} - {}",
                status,
                body,
            ));
        }

        let body = response.text().await?;
        let parsed: PendingRetrievesResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse pending retrieves response: {}. Raw: {}",
                e,
                body,
            )
        })?;

        Ok(parsed)
    }

    /// Report a C-MOVE retrieve as completed to Aurabox.
    ///
    /// Calls `POST {base_url}/api/bounce/retrieves/{id}/completed`.
    pub async fn post_retrieve_completed(&self, retrieve_id: &str) -> anyhow::Result<Value> {
        let url = format!(
            "{}/api/bounce/retrieves/{}/completed",
            self.base_url, retrieve_id,
        );

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({}))
            .send()
            .await?;

        Self::handle_response(response).await
    }

    /// Report a C-MOVE retrieve failure to Aurabox.
    ///
    /// Calls `POST {base_url}/api/bounce/retrieves/{id}/failed`.
    pub async fn post_retrieve_failed(
        &self,
        retrieve_id: &str,
        error: String,
    ) -> anyhow::Result<Value> {
        let url = format!(
            "{}/api/bounce/retrieves/{}/failed",
            self.base_url, retrieve_id,
        );

        let payload = RetrieveFailedPayload { error };

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

    /// Query the authenticated organisation's studies in Aura.
    ///
    /// Calls `POST {base_url}/api/bounce/find` with an optional set of DICOM
    /// filter attributes and returns the matching study list.
    ///
    /// This is used when Bounce acts as a C-FIND SCP: a connected SCU (e.g. a
    /// PACS workstation) sends a C-FIND request to Bounce, and Bounce proxies
    /// it into Aura's study database via this endpoint, then encodes the
    /// response back to the SCU as DICOM C-FIND response PDUs.
    pub async fn find_studies(
        &self,
        request: &FindStudyRequest,
    ) -> anyhow::Result<FindStudiesResponse> {
        let url = format!("{}/api/bounce/find", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to query Aura studies: HTTP {} - {}",
                status,
                body,
            ));
        }

        let body = response.text().await?;
        let parsed: FindStudiesResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse find studies response: {}. Raw: {}",
                e,
                body,
            )
        })?;

        Ok(parsed)
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
