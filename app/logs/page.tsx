'use client';

import {Suspense, useEffect, useState} from 'react'
import {useAppSelector} from "@/app/lib/hook";
import { openPath } from '@tauri-apps/plugin-opener';
import { dirname } from '@tauri-apps/api/path';
import { attachLogger } from '@tauri-apps/plugin-log'
import { appLogDir, join } from '@tauri-apps/api/path';
import { getName } from '@tauri-apps/api/app';
import { platform } from '@tauri-apps/plugin-os';

export default function Page() {

    const [loaded, setLoaded] = useState<boolean>(false);

    const logs = useAppSelector((state) => state.main.logs)

    const openLogPath = async (e: any) => {
        e.preventDefault()

        let logDirPath = await appLogDir();
        const currentPlatform = platform();
        const appName = await getName();

        let logFilePath;

        if (currentPlatform === 'windows') {
            // Windows: typically in %APPDATA%\[app-name]\logs\
            logFilePath = await join(logDirPath, `${appName}.log`);
        } else if (currentPlatform === 'macos') {
            // macOS: typically in ~/Library/Logs/[app-name]/
            logFilePath = await join(logDirPath, `${appName}.log`);
        } else {
            // Linux: typically in ~/.local/share/[app-name]/logs/
            logFilePath = await join(logDirPath, `${appName}.log`);
        }

        // Open the directory using the system's default file explorer
        await openPath(logFilePath);
    }

    const [backendLogs, setBackendLogs] = useState<string[]>([])

    attachLogger(({ message, level }): void => {
        setBackendLogs([...backendLogs, `[${level}] ${message}`])
    })

    useEffect(() => {
        setLoaded(true)
    }, []);

    return (
        <div className="h-full flex flex-col">
            <div className="bg-white shadow-lg rounded-lg p-6 w-full flex-grow">
                <button onClick={openLogPath}>open logs</button>
                <Suspense fallback={<Loading/>}>
                    {JSON.stringify(backendLogs)}
                    {(loaded ? <div className="h-full">
                        <ul className="overflow-y-scroll font-mono bg-stone-50 shadow-inner min-h-full p-1">
                            {logs.map((log, ind) => (
                                <li key={ind}>{log}</li>
                            ))}
                        </ul>
                    </div> : null)}
                </Suspense>
            </div>
        </div>
    );
}


function Loading() {
    return <h2>🌀 Loading...</h2>;
}