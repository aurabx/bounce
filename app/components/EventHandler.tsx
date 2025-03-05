'use client'

import { useEffect} from 'react'
import { logMessage, setRunning, setCurrentStudies } from '../lib/store'
import {useAppDispatch} from "@/app/lib/hook";
import {listen} from "@tauri-apps/api/event";
import {CurrentStudies, Study} from "@/app/lib/types";


export default function EventHandler({ children, }: { children: React.ReactNode }) {

    const dispatch = useAppDispatch()

    let bindEvents = async () => {
        await listen("log", (event) => {
            let action = logMessage(event.payload as string);
            dispatch(action)
        })

        await listen("running", (event) => {
            let action = setRunning(event.payload as boolean);
            dispatch(action)
        })

        await listen<CurrentStudies>('current-studies', (event) => {
            console.log(event.payload.studies);
            let action = setCurrentStudies(event.payload as CurrentStudies);
            dispatch(action)
        });
    }

    useEffect(() => {
        bindEvents().finally();
    }, [])

    return <>{children}</>
}