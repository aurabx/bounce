'use client'

import { useEffect} from 'react'
import { logMessage} from '../lib/store'
import {useAppDispatch} from "@/app/lib/hook";
import {listen} from "@tauri-apps/api/event";

export default function EventHandler({ children, }: { children: React.ReactNode }) {

    const dispatch = useAppDispatch()

    let bindEvents = async () => {
        await listen("log", (event) => {
            const log = logMessage(event.payload as string);
            dispatch(log)
        })
    }

    useEffect(() => {
        bindEvents().finally();
    }, [])

    return <>{children}</>
}