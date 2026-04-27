use crate::aura::query_api::QueryApiClient;
use crate::query::models::{
    CfindResult, FindStudiesResponse, FindStudyRequest, PendingJobsResponse,
    PendingQueriesResponse, ServicesResponse,
};
use crate::{load_config, log_info};
use anyhow::Error;
use reqwest::{Client, Response};
use serde_json::{json, Value};
use std::fs;
use tauri::AppHandle;

#[derive(Clone, Debug)]
pub struct AuraApi {
    client: Client,
    app_handle: AppHandle,
}

impl AuraApi {
    pub fn new(app: AppHandle) -> Self {
        Self {
            client: Client::new(),
            app_handle: app,
        }
    }

    /// Build a [`QueryApiClient`] from the current Tauri config.
    ///
    /// This resolves the base URL and API key from the Tauri Store so the
    /// underlying client can be constructed without a Tauri dependency.
    fn query_client(&self) -> QueryApiClient {
        let config = load_config(self.app_handle.clone());
        QueryApiClient::new(config.get_api_endpoint(), config.api_key)
    }

    pub async fn upload_config(&self) -> anyhow::Result<Value> {
        let config = load_config(self.app_handle.clone());

        let url = format!("{}/api/bounce/config", config.get_api_endpoint());

        println!("pulling uploader config (url: {})", url);

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", config.api_key))
            .send()
            .await?;

        log_info!("uploader config status {}", response.status());

        Self::handle_response(response).await
    }

    pub async fn upload_init(
        &self,
        study_uid: String,
        signature: String,
        upload_id: String,
    ) -> anyhow::Result<Value> {
        println!(
            "upload_start (study_uid: {}), (signature: {}), (signature: {})",
            study_uid, signature, upload_id
        );

        let config = load_config(self.app_handle.clone());
        let json_path = config.resolve_metadata_path(&study_uid);

        let json_content = fs::read_to_string(&json_path)?;
        let json_value: Value = serde_json::from_str::<Value>(&json_content)?;

        // Check for a retrieve marker file. If present, the study was
        // retrieved via C-MOVE and we know the Aura patient_id to include
        // in the upload init so the transfer is auto-matched to the patient.
        let retrieve_marker_path = config.resolve_retrieve_marker_path(&study_uid);

        log_info!(
            "upload_init: checking for retrieve marker at {:?} (exists={})",
            retrieve_marker_path,
            retrieve_marker_path.exists(),
        );

        let patient_id = fs::read_to_string(&retrieve_marker_path).ok();

        if let Some(ref pid) = patient_id {
            log_info!(
                "upload_init: found retrieve marker for study {}, patient_id={}",
                study_uid,
                pid,
            );
        } else {
            log_info!(
                "upload_init: no retrieve marker found for study {}",
                study_uid,
            );
        }

        let url = format!("{}/api/bounce/upload/init", config.get_api_endpoint());

        let mut payload = json!({
            "studies" : json_value.get("studies").unwrap(),
            "mode" : "bulk",
            "type": "lift",
            "signature" : signature,
            "upload_id" : upload_id,
        });

        if let Some(pid) = &patient_id {
            payload
                .as_object_mut()
                .unwrap()
                .insert("patient_id".to_string(), Value::String(pid.clone()));
        }

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .header("Content-Type", "application/json")
            .header("Accepts", "application/json")
            .header("Authorization", format!("Bearer {}", config.api_key))
            .send()
            .await?;

        Self::handle_response(response).await
    }

    pub async fn upload_save(
        &self,
        upload_id: String,
        assembly_id: String,
        method: &str,
    ) -> anyhow::Result<Value> {
        println!(
            "upload_update (upload_id: {}), (assembly_id: {})",
            upload_id, assembly_id
        );

        let config = load_config(self.app_handle.clone());
        let path = match method {
            "complete" => "complete".to_string(),
            _ => "start".to_string(),
        };

        let url = format!("{}/api/bounce/upload/{}", config.get_api_endpoint(), path);

        log_info!("upload_save url {}", url);
        log_info!("upload_save assembly_id {}", assembly_id);
        log_info!("upload_save upload_id {}", upload_id);

        let response = self
            .client
            .post(&url)
            .json(&json!({
                "assembly_id" : assembly_id,
                "type": "lift",
                "upload_id" : upload_id,
            }))
            .header("Content-Type", "application/json")
            .header("Accepts", "application/json")
            .header("Authorization", format!("Bearer {}", config.api_key))
            .send()
            .await?;

        Self::handle_response(response).await
    }

    // -----------------------------------------------------------------------
    // C-FIND query polling endpoints
    //
    // These delegate to [`QueryApiClient`] which contains the HTTP logic
    // and can be tested independently with a mock HTTP server.
    // -----------------------------------------------------------------------

    /// Fetch pending C-FIND queries from Aurabox for this gateway.
    pub async fn fetch_pending_queries(&self) -> anyhow::Result<PendingQueriesResponse> {
        self.query_client().fetch_pending_queries().await
    }

    /// Fetch configured DICOM services (remote PACS) from Aurabox.
    pub async fn fetch_services(&self) -> anyhow::Result<ServicesResponse> {
        self.query_client().fetch_services().await
    }

    /// Post C-FIND results back to Aurabox.
    pub async fn post_query_results(
        &self,
        query_id: &str,
        results: Vec<CfindResult>,
    ) -> anyhow::Result<Value> {
        log_info!("Posting C-FIND results for query {}", query_id);
        self.query_client()
            .post_query_results(query_id, results)
            .await
    }

    /// Query Aura's study database on behalf of an inbound C-FIND from a
    /// connected SCU.
    ///
    /// Calls `POST /api/bounce/find` and returns the matching studies.
    #[allow(dead_code)] // called from dicom_server at runtime
    pub async fn find_studies(
        &self,
        request: &FindStudyRequest,
    ) -> anyhow::Result<FindStudiesResponse> {
        self.query_client().find_studies(request).await
    }

    /// Report a C-FIND query failure to Aurabox.
    pub async fn post_query_failed(&self, query_id: &str, error: String) -> anyhow::Result<Value> {
        log_info!("Posting C-FIND failure for query {}", query_id);
        self.query_client().post_query_failed(query_id, error).await
    }

    // -------------------------------------------------------------------
    // Unified jobs polling endpoint
    //
    // Returns a tagged-union list of retrieves and sends in one call.
    // -------------------------------------------------------------------

    /// Fetch all pending jobs (retrieves + sends) for this gateway.
    pub async fn fetch_pending_jobs(&self) -> anyhow::Result<PendingJobsResponse> {
        self.query_client().fetch_pending_jobs().await
    }

    // -------------------------------------------------------------------
    // C-MOVE retrieve lifecycle endpoints
    // -------------------------------------------------------------------

    /// Report a C-MOVE retrieve as completed to Aurabox.
    pub async fn post_retrieve_completed(&self, retrieve_id: &str) -> anyhow::Result<Value> {
        log_info!("Posting C-MOVE retrieve completed for {}", retrieve_id);
        self.query_client()
            .post_retrieve_completed(retrieve_id)
            .await
    }

    /// Report a C-MOVE retrieve failure to Aurabox.
    pub async fn post_retrieve_failed(
        &self,
        retrieve_id: &str,
        error: String,
    ) -> anyhow::Result<Value> {
        log_info!("Posting C-MOVE retrieve failure for {}", retrieve_id);
        self.query_client()
            .post_retrieve_failed(retrieve_id, error)
            .await
    }

    // -------------------------------------------------------------------
    // C-STORE send lifecycle endpoints
    // -------------------------------------------------------------------

    /// Report mid-send progress to Aurabox.
    pub async fn post_send_progress(
        &self,
        send_id: &str,
        instances_sent: u32,
        instance_count: Option<u32>,
    ) -> anyhow::Result<Value> {
        self.query_client()
            .post_send_progress(send_id, instances_sent, instance_count)
            .await
    }

    /// Mark a send as completed.
    pub async fn post_send_completed(&self, send_id: &str) -> anyhow::Result<Value> {
        log_info!("Posting C-STORE send completed for {}", send_id);
        self.query_client().post_send_completed(send_id).await
    }

    /// Report a send failure to Aurabox.
    pub async fn post_send_failed(&self, send_id: &str, error: String) -> anyhow::Result<Value> {
        log_info!("Posting C-STORE send failure for {}", send_id);
        self.query_client().post_send_failed(send_id, error).await
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
                status, error_detail
            );

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
