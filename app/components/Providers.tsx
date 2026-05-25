'use client'
import { useRef } from 'react'
import { Provider } from 'react-redux'
import { makeStore, AppStore } from '../lib/store'
import { UpdateProvider } from '@/app/lib/UpdateContext'

export default function Providers({ children, }: { children: React.ReactNode }) {
    const storeRef = useRef<AppStore>()

    if (!storeRef.current) {
        // Create the store instance the first time this renders
        storeRef.current = makeStore()
    }

    return (
        <Provider store={storeRef.current}>
            <UpdateProvider>{children}</UpdateProvider>
        </Provider>
    )
}