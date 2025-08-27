
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