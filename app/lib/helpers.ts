import { DateTime } from "luxon";

export const classNames = (...classes: any[])=> {
    return classes.filter(Boolean).join(' ')
}

export const formatDicomDateAndTime = (dicomDate: string, dicomTime: string) => {
    // "study_date": "20020628",
    // "study_time": "160956.0",

    const processedDicomTime = parseInt(dicomTime)

    return DateTime.fromFormat(dicomDate + ' ' + processedDicomTime, 'yyyyMMdd HHmmss').toFormat('FFF')
}

