'use client'

import { useEffect} from 'react'
import {logMessage, setRunning, setCurrentStudies, setRunningDetail} from '../lib/store'
import {useAppDispatch} from "@/app/lib/hook";
import {listen} from "@tauri-apps/api/event";
import {CurrentStudies, Study} from "@/app/lib/types";
import {invoke} from "@tauri-apps/api/core";


export default function EventHandler({ children, }: { children: React.ReactNode }) {

    const dispatch = useAppDispatch()

    const bindEvents = async () => {
        await listen("log", (event) => {
            let action = logMessage(event.payload as string);
            dispatch(action)
        })

        await listen("running", (event) => {
            let action = setRunning(event.payload as boolean);
            dispatch(action)
        })

        await listen("running-details", (event) => {
            let action = setRunningDetail(event.payload as string);
            dispatch(action)
        })

        await listen<CurrentStudies>('current-studies', (event) => {
            let action = setCurrentStudies(event.payload as CurrentStudies);
            dispatch(action)
        });
    }

    const initialEvents = async () => {
        await invoke('current_studies');
    }

    useEffect(() => {
        bindEvents()
            .then(() => {
                initialEvents().catch(console.error)
            })
            .catch(console.error);
    }, [])

    return <>{children}</>
}