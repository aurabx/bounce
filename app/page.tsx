'use client';

import {useEffect, useState} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { logMessage } from "./lib/store"
import {useSelector} from "react-redux";
import {useAppDispatch, useAppSelector} from "@/app/lib/hook";
import { listen } from '@tauri-apps/api/event';

export default function Page() {
    const [port, setPort] = useState<string>('104'); // Default port
    //const logs = useSelector(selectLogs);

    const logs = useAppSelector((state) => state.main.logs)
    const dispatch = useAppDispatch()

    let bindEvents = async () => {
        await listen("log", (event) => {
            console.log(event)
            const log = logMessage(event.payload as string);
            dispatch(log)
        })
    }

    useEffect(() => {
        bindEvents().finally();
    }, [])




    const startService = async () => {
        try {
            await invoke('start_service', { port: parseInt(port) }); // Pass the port to Tauri
            console.info(`Dicom server started on port ${port}`);
        } catch (error) {
            console.error('Error starting server:', error);
            alert(`Failed to start server: ${error}`);
        }
    };

    const sendLog = async () => {
        try {
            await invoke('send_log', { log: "Log and things" }); // Pass the port to Tauri
        } catch (error) {
            console.error('Error starting server:', error);
            alert(`Failed to start server: ${error}`);
        }
    };



    return (
        <main className="">
            <h1 className="text-3xl font-bold mb-6">
                Aurabox Bounce
            </h1>
            <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-6 w-full max-w-lg">
                <div className="mb-4">
                    <label htmlFor="port" className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                        Port Number
                    </label>
                    <input
                        type="number"
                        id="port"
                        value={port}
                        onChange={(e) => setPort(e.target.value)}
                        className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                        placeholder="Enter port number"
                    />
                </div>
                <div className="mb-4">
                    <button
                        onClick={startService}
                        className="flex w-full  transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                    >
                        Start Service
                    </button>
                </div>
                <div className="mb-4">
                    <button
                        onClick={sendLog}
                        className="flex w-full  transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                    >
                        Send Log
                    </button>
                </div>
            </div>
            <div className="bg-white mt-4 shadow-lg rounded-lg p-6 w-full max-w-lg overflow-auto">
                {JSON.stringify(logs)}
            </div>

        </main>
    );
}
