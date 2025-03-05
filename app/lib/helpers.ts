import { DateTime } from "luxon";

export const classNames = (...classes: any[])=> {
    return classes.filter(Boolean).join(' ')
}


export const formatDicomDateAndTime = (dicomDate: string, dicomTime: string) => {
    return DateTime.fromFormat(dicomDate + ' ' + dicomTime, 'yyyyMMdd HHmmss').toFormat('FFF')
}