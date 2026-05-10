//! Testable HTTP client for C-FIND query-related Aurabox endpoints.
//!
//! [`QueryApiClient`] encapsulates the HTTP request/response logic for the
//! query polling endpoints without requiring a Tauri `AppHandle`. This allows
//! the methods to be tested against a mock HTTP server.
//!
//! The main [`super::aura_api::AuraApi`] delegates to this client, resolving
//! the `base_url` and `api_key` from the Tauri config store.

use crate::log_info;
use crate::query::models::{
    CfindResult, FindStudiesResponse, FindStudyRequest, MoveResolveRequest,
    MoveResolveResponse, PendingJobsResponse, PendingQueriesResponse, QueryFailedPayload,
    QueryResultsPayload, RetrieveFailedPayload, SendFailedPayload, SendProgressPayload,
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

/// Outcome of [`QueryApiClient::resolve_move`].
///
/// Aura's `move/resolve` endpoint distinguishes three success-shaped paths
/// that the SCP handler responds to differently:
///   - [`MoveResolveOutcome::Resolved`] — proceed with the retrieve.
///   - [`MoveResolveOutcome::NotFound`] — surface as DIMSE status 0xA900
///     (Identifier does not match SOP Class) or similar to the SCU.
///   - [`MoveResolveOutcome::DestinationUnknown`] — surface as DIMSE status
///     0xA801 (Move destination unknown) to the SCU.
#[derive(Debug, Clone)]
pub enum MoveResolveOutcome {
    Resolved(MoveResolveResponse),
    NotFound,
    DestinationUnknown,
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
            anyhow::anyhow!("Failed to parse services response: {}. Raw: {}", e, body,)
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
        let url = format!("{}/api/bounce/queries/{}/results", self.base_url, query_id,);

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
    pub async fn post_query_failed(&self, query_id: &str, error: String) -> anyhow::Result<Value> {
        let url = format!("{}/api/bounce/queries/{}/failed", self.base_url, query_id,);

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
    // Unified jobs endpoint
    // -------------------------------------------------------------------

    /// Fetch all pending jobs (retrieves and sends) for this gateway.
    ///
    /// Calls `GET {base_url}/api/bounce/jobs/pending`. The response is a
    /// tagged-union list discriminated by `type` — see [`crate::query::models::Job`].
    pub async fn fetch_pending_jobs(&self) -> anyhow::Result<PendingJobsResponse> {
        let url = format!("{}/api/bounce/jobs/pending", self.base_url);

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
                "Failed to fetch pending jobs: HTTP {} - {}",
                status,
                body,
            ));
        }

        let body = response.text().await?;
        let parsed: PendingJobsResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse pending jobs response: {}. Raw: {}",
                e,
                body,
            )
        })?;

        Ok(parsed)
    }

    // -------------------------------------------------------------------
    // C-MOVE retrieve lifecycle endpoints
    // -------------------------------------------------------------------

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

    // -------------------------------------------------------------------
    // C-STORE send lifecycle endpoints
    // -------------------------------------------------------------------

    /// Report mid-send progress to Aurabox.
    ///
    /// Calls `POST {base_url}/api/bounce/sends/{id}/progress`.
    pub async fn post_send_progress(
        &self,
        send_id: &str,
        instances_sent: u32,
        instance_count: Option<u32>,
    ) -> anyhow::Result<Value> {
        let url = format!("{}/api/bounce/sends/{}/progress", self.base_url, send_id);

        let payload = SendProgressPayload {
            instances_sent,
            instance_count,
        };

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

    /// Mark a send as completed once every instance has been C-STOREd.
    ///
    /// Calls `POST {base_url}/api/bounce/sends/{id}/completed`.
    pub async fn post_send_completed(&self, send_id: &str) -> anyhow::Result<Value> {
        let url = format!("{}/api/bounce/sends/{}/completed", self.base_url, send_id);

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

    /// Report a send failure to Aurabox.
    ///
    /// Calls `POST {base_url}/api/bounce/sends/{id}/failed`.
    pub async fn post_send_failed(&self, send_id: &str, error: String) -> anyhow::Result<Value> {
        let url = format!("{}/api/bounce/sends/{}/failed", self.base_url, send_id);

        let payload = SendFailedPayload { error };

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
    // C-MOVE / C-GET SCP — workstation-initiated retrieves from Aurabox
    // -------------------------------------------------------------------

    /// Resolve a study UID to a WADO-RS source (and optionally a configured
    /// move-destination service) so Bounce can serve a workstation's
    /// C-MOVE-RQ or C-GET-RQ.
    ///
    /// Calls `POST {base_url}/api/bounce/move/resolve`. A 404 response is
    /// surfaced as `MoveResolveOutcome::NotFound`, a 422 (move destination AE
    /// unknown) as `MoveResolveOutcome::DestinationUnknown`. Anything else is
    /// returned as a regular error.
    pub async fn resolve_move(
        &self,
        request: &MoveResolveRequest,
    ) -> anyhow::Result<MoveResolveOutcome> {
        let url = format!("{}/api/bounce/move/resolve", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(request)
            .send()
            .await?;

        let status = response.status();

        if status.as_u16() == 404 {
            return Ok(MoveResolveOutcome::NotFound);
        }

        if status.as_u16() == 422 {
            return Ok(MoveResolveOutcome::DestinationUnknown);
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to resolve move: HTTP {} - {}",
                status,
                body,
            ));
        }

        let body = response.text().await?;
        let parsed: MoveResolveResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse move/resolve response: {}. Raw: {}",
                e,
                body,
            )
        })?;

        Ok(MoveResolveOutcome::Resolved(parsed))
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

        log_info!("Aura query: POST {} filters={:?}", url, request,);

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(request)
            .send()
            .await?;

        log_info!(
            "Aura query: response status {} from {}",
            response.status(),
            url
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            log_info!(
                "Aura query: non-success response body from {} => {}",
                url,
                body,
            );
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

        log_info!(
            "Aura query: parsed {} studies from {}",
            parsed.studies.len(),
            url,
        );

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
