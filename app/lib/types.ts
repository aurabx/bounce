
export interface Study {
    study_uid: string,
    study_description: string
}

export type CurrentStudies = {
    studies: Study[]
}