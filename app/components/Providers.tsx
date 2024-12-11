'use client'
import { useRef } from 'react'
import { Provider } from 'react-redux'
import {makeStore, AppStore, logMessage} from '../lib/store'
import {useAppDispatch, useAppSelector} from "@/app/lib/hook";
import {listen} from "@tauri-apps/api/event";

export default function Providers({ children, }: { children: React.ReactNode }) {
    const storeRef = useRef<AppStore>()

    if (!storeRef.current) {
        // Create the store instance the first time this renders
        storeRef.current = makeStore()
    }



    return <Provider store={storeRef.current}>{children}</Provider>
}