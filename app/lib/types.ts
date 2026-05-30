
export interface Study {
    study_uid: string,
    study_description: string,
    study_date: string,
    study_time: string,
    status: string,
    exists: boolean,
    patient_name?: string,
    patient_id?: string,
    accession_no?: string,
    series_count?: number,
    images?: number,
    attempts?: number,
    last_attempt_at?: string | null,
    next_retry_at?: string | null,
    last_error?: string | null,
    created_at?: string,
    updated_at?: string,
    sent_at?: string | null,
}

export interface Pagination {
    current_page: number,
    total_pages: number,
    total_items: number,
    limit: number,
    offset: number,
    has_next_page: boolean,
    has_previous_page: boolean,
    items_on_page: number,
    search: string | null,
}

export type CurrentStudies = {
    studies: Study[],
    pagination: Pagination,
}

// A single upload attempt for a study, surfaced in the UI as a
// "transaction". Mirrors the backend `UploadAttempt` struct
// (src-tauri/src/db/models.rs); timestamps arrive as ISO-8601 strings.
export interface UploadAttempt {
    id: number | null,
    study_uid: string,
    attempt_no: number,
    upload_id?: string | null,
    status: string,
    error?: string | null,
    started_at: string,
    finished_at?: string | null,
    duration_ms?: number | null,
}

export type CurrentUploadAttempts = {
    attempts: UploadAttempt[],
    pagination: Pagination,
}

export interface DicomService {
    id: string,
    label: string,
    ae_title: string,
    host: string,
    port: number,
}

export interface EchoResult {
    ok: boolean,
    latency_ms: number | null,
    error: string | null,
}
