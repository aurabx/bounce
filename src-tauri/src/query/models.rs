use serde::{Deserialize, Serialize};

/// PACS connection details provided by Aurabox for each query.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PacsService {
    pub ae_title: String,
    pub host: String,
    pub port: u16,
}

/// Filter criteria for a C-FIND query.
///
/// Populated fields act as match keys; `None` fields are omitted from the
/// query identifier (the PACS will not filter on them but may still return
/// the corresponding tag values as return keys).
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct QueryFilters {
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub study_date: Option<String>,
    pub accession_number: Option<String>,
    pub modality: Option<String>,
}

/// A single pending query request fetched from Aurabox.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PacsQueryRequest {
    pub id: String,
    pub service: PacsService,
    pub query_level: String,
    pub filters: QueryFilters,
}

/// The wrapper returned by `GET /api/bounce/queries/pending`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PendingQueriesResponse {
    pub queries: Vec<PacsQueryRequest>,
}

/// A single result from a C-FIND response.
///
/// Contains fields for both STUDY-level and PATIENT-level queries.
/// Study-level fields (`study_date`, `study_instance_uid`, etc.) are
/// `None` for PATIENT-level results, and patient-level fields
/// (`patient_birth_date`, `patient_sex`, etc.) are `None` for
/// STUDY-level results.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CfindResult {
    // Common fields (present at both levels)
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,

    // Study-level fields
    pub study_date: Option<String>,
    pub study_time: Option<String>,
    pub study_description: Option<String>,
    pub accession_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_instance_uid: Option<String>,
    pub modalities_in_study: Option<String>,
    pub number_of_series: Option<u32>,
    pub number_of_instances: Option<u32>,

    // Patient-level fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_birth_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_sex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_patient_related_studies: Option<u32>,
}

/// Payload sent to `POST /api/bounce/queries/{id}/results`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryResultsPayload {
    pub results: Vec<CfindResult>,
}

/// A DICOM service (remote PACS) configured in Aurabox.
///
/// Returned by `GET /api/bounce/queries/services`. This extends the
/// connection-only [`PacsService`] with an Aurabox `id` and a human-readable
/// `label` so the Bounce UI can display available PACS endpoints.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DicomService {
    pub id: String,
    pub label: String,
    pub ae_title: String,
    pub host: String,
    pub port: u16,
}

/// Convert a [`DicomService`] into the lighter [`PacsService`] used by C-FIND
/// execution, discarding the Aurabox-specific `id` and `label` fields.
impl From<DicomService> for PacsService {
    fn from(svc: DicomService) -> Self {
        PacsService {
            ae_title: svc.ae_title,
            host: svc.host,
            port: svc.port,
        }
    }
}

/// The wrapper returned by `GET /api/bounce/queries/services`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServicesResponse {
    pub services: Vec<DicomService>,
}

/// Payload sent to `POST /api/bounce/queries/{id}/failed`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryFailedPayload {
    pub error: String,
}
