'use client'

import { useEffect} from 'react'
import { attachLogger, LogLevel } from '@tauri-apps/plugin-log'
import {logMessage, setRunning, setCurrentStudies, setRunningDetail, setErrors, setError} from '../lib/store'
import {useAppDispatch} from "@/app/lib/hook";
import {listen} from "@tauri-apps/api/event";
import {CurrentStudies} from "@/app/lib/types";
import {invokeCommand} from "@/app/lib/commands";
import {load} from "@tauri-apps/plugin-store";


export default function EventHandler({ children, }: { children: React.ReactNode }) {

    const dispatch = useAppDispatch()

    const normalizeLogLevel = (level: LogLevel): 'trace' | 'debug' | 'info' | 'warn' | 'error' => {
        switch (level) {
            case LogLevel.Trace:
                return 'trace'
            case LogLevel.Debug:
                return 'debug'
            case LogLevel.Warn:
                return 'warn'
            case LogLevel.Error:
                return 'error'
            case LogLevel.Info:
            default:
                return 'info'
        }
    }

    const createLogEntry = (
        message: string,
        source: 'event' | 'system',
        level: 'trace' | 'debug' | 'info' | 'warn' | 'error' = 'info',
    ) => ({
        id: `${source}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        level,
        message,
        source,
        timestamp: new Date().toISOString(),
    })

    useEffect(() => {
        let cleanup: (() => void) | undefined

        const bindEvents = async () => {
            // Manual emit("log", ...) calls from Rust (server start/stop, association events)
            const unlistenLogEvent = await listen("log", (event) => {
                dispatch(logMessage(createLogEntry(event.payload as string, 'event', 'info')))
            })

            // tauri_plugin_log webview target — receives all log::info!/log::error! records
            const detachLogger = await attachLogger((entry) => {
                dispatch(logMessage(createLogEntry(entry.message, 'system', normalizeLogLevel(entry.level))))
            })

            const unlistenRunning = await listen("running", (event) => {
                let action = setRunning(event.payload as boolean);
                dispatch(action)
            })

            const unlistenRunningDetails = await listen("running-details", (event) => {
                let action = setRunningDetail(event.payload as string);
                dispatch(action)
            })

            const unlistenCurrentStudies = await listen<CurrentStudies>('current-studies', (event) => {
                let action = setCurrentStudies(event.payload as CurrentStudies);
                dispatch(action)
            });

            const unlistenError = await listen('error', (event) => {
                let action = setError(event.payload as string);
                dispatch(action)

                setTimeout(() => {
                    let action = setErrors([]);
                    dispatch(action)
                }, 5000)
            });

            return () => {
                unlistenLogEvent()
                detachLogger()
                unlistenRunning()
                unlistenRunningDetails()
                unlistenCurrentStudies()
                unlistenError()
            }
        }

        // If the previous session was restarted (e.g. by an automatic update)
        // while the receiver was running, restore that state by starting the
        // service again. The flag is cleared before starting so a failed start
        // cannot cause a restart loop.
        const resumeRunningIfNeeded = async () => {
            try {
                const store = await load('store.json', { autoSave: false })
                const resume = await store.get('_resume_running')
                if (resume === true) {
                    await store.set('_resume_running', false)
                    await store.save()
                    await invokeCommand('receiver_start')
                }
            } catch (e) {
                console.error('Failed to resume running state', e)
            }
        }

        bindEvents()
            .then(async (dispose) => {
                cleanup = dispose
                await resumeRunningIfNeeded()
                try {
                    await invokeCommand('current_studies')
                } catch (e) {
                    dispatch(setError(`Failed to load studies: ${e}`))
                }
            })
            .catch((e) => {
                dispatch(setError(`Failed to initialise event listeners: ${e}`))
            });

        return () => {
            cleanup?.()
        }
    }, [dispatch])

    return <>{children}</>
}
