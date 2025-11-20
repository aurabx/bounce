'use client';

import {Suspense, useEffect, useState} from 'react'
import {useAppSelector} from "@/app/lib/hook";
import { openPath } from '@tauri-apps/plugin-opener';
import { attachLogger } from '@tauri-apps/plugin-log'
import { appLogDir, join } from '@tauri-apps/api/path';
import { getName } from '@tauri-apps/api/app';
import { platform } from '@tauri-apps/plugin-os';
import { Card, CardContent } from "@/app/components/ui/card";
import { Button } from "@/app/components/ui/button";
import { ScrollArea } from "@radix-ui/react-scroll-area"; // Assuming we might want to use ScrollArea later, but normal div overflow is fine too.

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

    useEffect(() => {
        setLoaded(true)
    }, []);

    return (
        <div className="h-full flex flex-col space-y-4">
             <div className="flex justify-end">
                 <Button variant="outline" onClick={openLogPath}>Open Log File</Button>
             </div>
            <Card className="flex-grow overflow-hidden flex flex-col">
                <CardContent className="p-0 flex-grow bg-muted/30 font-mono text-xs sm:text-sm relative">
                    <Suspense fallback={<Loading/>}>
                        {(loaded ? <div className="absolute inset-0 overflow-y-auto p-4">
                            <ul className="space-y-1">
                                {logs.map((log, ind) => (
                                    <li key={ind} className="break-all whitespace-pre-wrap border-b border-border/50 last:border-0 pb-1 mb-1">{log}</li>
                                ))}
                            </ul>
                        </div> : null)}
                    </Suspense>
                </CardContent>
            </Card>
        </div>
    );
}

function Loading() {
    return <div className="p-4">Loading...</div>;
}
