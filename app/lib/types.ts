
export interface Study {
    study_uid: string,
    study_description: string
    study_date: string,
    study_time: string,
    status: string,
    exists: string,
}

export type CurrentStudies = {
    studies: Study[]
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