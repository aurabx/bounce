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

        // Decide whether to auto-start the DICOM receiver on launch.
        //
        // Two independent reasons trigger an auto-start:
        //   1. `_resume_running` — a one-shot flag set on an in-app relaunch
        //      (Settings → Restart Now, tray → Relaunch, automatic update)
        //      when the receiver was running, so the service comes back in
        //      the same state after the restart.
        //   2. `start_receiver_on_start` — a user preference that asks Bounce
        //      to always start the receiver when the app launches, including
        //      after a manual quit. Without this, a manual quit + restart
        //      leaves the receiver stopped until the operator presses Start.
        //
        // The one-shot flag is cleared before invoking `receiver_start` so a
        // failing start cannot cause a restart loop. Only one start command
        // is issued even when both conditions are true.
        const startReceiverIfRequired = async () => {
            try {
                const store = await load('store.json', { autoSave: false, defaults: {} })
                const resume = (await store.get('_resume_running')) === true
                const autoStart = (await store.get('start_receiver_on_start')) === 'yes'

                if (resume) {
                    await store.set('_resume_running', false)
                    await store.save()
                }

                if (resume || autoStart) {
                    await invokeCommand('receiver_start')
                }
            } catch (e) {
                console.error('Failed to auto-start receiver', e)
            }
        }

        bindEvents()
            .then(async (dispose) => {
                cleanup = dispose
                await startReceiverIfRequired()
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
