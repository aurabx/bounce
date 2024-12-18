'use client';

import { invoke } from '@tauri-apps/api/core';
import {receiverStart, receiverStop} from "@/app/lib/server";


export default function Page() {


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
            <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-6 w-full">
                <div className="mb-4">
                    <button
                        onClick={receiverStart}
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
        </main>
    );
}
