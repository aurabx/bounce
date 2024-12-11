'use client';

import {Suspense, useEffect, useState} from 'react'
import { listen } from "@tauri-apps/api/event";

export default function Page() {

    const [loaded, setLoaded] = useState<boolean>(false);
    const [log, setLog] = useState<string>('');


    let bindEvents = async () => {
        await listen("log", (event) => {

        })
    }

    bindEvents()

    return (
        <>
            <h1 className="text-3xl font-bold mb-6">
                Logs
            </h1>
            <div className="bg-white shadow-lg rounded-lg p-6 w-full">
                <Suspense fallback={<Loading />}>
                    {(loaded ? <div className="min-h-screen">
                        <textarea className="min-h-screen">{log}</textarea>
                    </div> : null)}
                </Suspense>
            </div>
        </>
    );
}

function Loading() {
    return <h2>🌀 Loading...</h2>;
}