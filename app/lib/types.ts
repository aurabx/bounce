
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
