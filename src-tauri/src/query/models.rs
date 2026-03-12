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
    pub study_instance_uid: Option<String>,
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
/// Contains fields for STUDY-level, SERIES-level, and PATIENT-level queries.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    pub modalities_in_study: Option<String>,
    pub number_of_series: Option<u32>,
    pub number_of_instances: Option<u32>,

    // Series-level fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_instance_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_part_examined: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub laterality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub institution_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referring_physician_name: Option<String>,

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

// ---------------------------------------------------------------------------
// C-MOVE retrieve models
// ---------------------------------------------------------------------------

/// PACS service details included in a retrieve request.
///
/// Extends [`PacsService`] with an Aurabox service `id` so Bounce can
/// reference the service when reporting back.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RetrieveService {
    pub id: String,
    pub ae_title: String,
    pub host: String,
    pub port: u16,
}

/// Convert a [`RetrieveService`] into the lighter [`PacsService`] used by
/// C-MOVE execution, discarding the Aurabox-specific `id` field.
impl From<RetrieveService> for PacsService {
    fn from(svc: RetrieveService) -> Self {
        PacsService {
            ae_title: svc.ae_title,
            host: svc.host,
            port: svc.port,
        }
    }
}

/// A single pending retrieve request fetched from Aurabox.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PacsRetrieveRequest {
    pub id: String,
    pub service: RetrieveService,
    pub study_instance_uid: String,
    pub patient_id: Option<String>,
}

/// The wrapper returned by `GET /api/bounce/retrieves/pending`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PendingRetrievesResponse {
    pub retrieves: Vec<PacsRetrieveRequest>,
}

/// Payload sent to `POST /api/bounce/retrieves/{id}/failed`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RetrieveFailedPayload {
    pub error: String,
}

// ---------------------------------------------------------------------------
// Aura-find models (POST /api/bounce/find)
// ---------------------------------------------------------------------------

/// Request body sent to `POST /api/bounce/find`.
///
/// All fields are optional. Omitting a field means "no filter on this
/// attribute" (return all values). Field values follow DICOM wildcard
/// conventions: `*` = any sequence, `?` = single character.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FindStudyRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accession_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_instance_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_description: Option<String>,
    /// Maximum results to return. Capped by Aura at 200.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl FindStudyRequest {
    /// Build a `FindStudyRequest` from a `QueryFilters` value.
    ///
    /// Only STUDY-level filters are supported; the `limit` field is left at
    /// its server-side default (200) unless the caller sets it explicitly.
    pub fn from_filters(filters: &QueryFilters) -> Self {
        Self {
            patient_name: filters.patient_name.clone(),
            patient_id: filters.patient_id.clone(),
            study_date: filters.study_date.clone(),
            accession_number: filters.accession_number.clone(),
            modality: filters.modality.clone(),
            study_instance_uid: filters.study_instance_uid.clone(),
            study_description: None,
            limit: None,
        }
    }
}

/// A single study returned by `POST /api/bounce/find`.
///
/// Field names mirror the DICOM C-FIND STUDY-level response shape used
/// by `PacsQueryResultsRequest` on the Aura side.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FindStudyResult {
    pub study_instance_uid: Option<String>,
    pub study_date: Option<String>,
    pub study_time: Option<String>,
    pub study_description: Option<String>,
    pub accession_number: Option<String>,
    pub modalities_in_study: Option<serde_json::Value>,
    pub number_of_series: Option<u32>,
    pub number_of_instances: Option<u32>,
    pub institution_name: Option<String>,
    pub referring_physician_name: Option<String>,
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub patient_birth_date: Option<String>,
    pub patient_sex: Option<String>,
}

impl FindStudyResult {
    /// Convert an Aura find result into a [`CfindResult`] that Bounce can
    /// encode into DICOM C-FIND response PDUs.
    pub fn into_cfind_result(self) -> CfindResult {
        // Aura returns modalities_in_study as an array (e.g. ["CT","MR"]).
        // DICOM represents it as a backslash-separated string.
        let modalities_in_study = match &self.modalities_in_study {
            Some(serde_json::Value::Array(arr)) => {
                let joined: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if joined.is_empty() {
                    None
                } else {
                    Some(joined.join("\\"))
                }
            }
            Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        CfindResult {
            patient_name: self.patient_name,
            patient_id: self.patient_id,
            study_date: self.study_date,
            study_time: self.study_time,
            study_description: self.study_description,
            accession_number: self.accession_number,
            study_instance_uid: self.study_instance_uid,
            modality: None,
            modalities_in_study,
            number_of_series: self.number_of_series,
            number_of_instances: self.number_of_instances,
            series_instance_uid: None,
            series_description: None,
            series_number: None,
            series_date: None,
            series_time: None,
            body_part_examined: None,
            laterality: None,
            institution_name: self.institution_name,
            referring_physician_name: self.referring_physician_name,
            patient_birth_date: self.patient_birth_date,
            patient_sex: self.patient_sex,
            number_of_patient_related_studies: None,
        }
    }
}

/// Response envelope returned by `POST /api/bounce/find`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FindStudiesResponse {
    pub studies: Vec<FindStudyResult>,
}
