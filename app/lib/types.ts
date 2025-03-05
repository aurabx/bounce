
export interface Study {
    study_uid: string,
    study_description: string
    study_date: string,
}

export type CurrentStudies = {
    studies: Study[]
}