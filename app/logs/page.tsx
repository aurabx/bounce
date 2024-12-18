'use client';

import {Suspense, useEffect, useState} from 'react'
import {useAppSelector} from "@/app/lib/hook";

export default function Page() {

    const [loaded, setLoaded] = useState<boolean>(false);

    const logs = useAppSelector((state) => state.main.logs)

    useEffect(() => {
        setLoaded(true)
    }, []);

    return (
        <div className="h-full flex flex-col">
            <h1 className="text-3xl font-bold mb-6">
                Logs
            </h1>
            <div className="bg-white shadow-lg rounded-lg p-6 w-full flex-grow">
                <Suspense fallback={<Loading/>}>
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