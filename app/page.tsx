'use client';

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export default function Home() {
    const [port, setPort] = useState<string>('8080'); // Default port

    const startServer = async () => {
        try {
            await invoke('start_server', { port: parseInt(port) }); // Pass the port to Tauri
            alert(`TCP Server started on port ${port}`);
        } catch (error) {
            console.error('Error starting server:', error);
            alert(`Failed to start server: ${error}`);
        }
    };

    const startService = async () => {
        try {
            await invoke('start_service'); // Pass the port to Tauri
            alert(`Dicom server started`);
            console.info('Dicom server started');
        } catch (error) {
            console.error('Error starting server:', error);
            alert(`Failed to start server: ${error}`);
        }
    };

    return (
        <main className="flex min-h-screen flex-col items-center justify-center p-6">
            <h1 className="text-3xl font-bold mb-6 text-center">
                Aurabox Proxy TCP Server
            </h1>
            <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-6 w-full max-w-md">
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
                        onClick={startServer}
                        className="w-full bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-400"
                    >
                        Start Server
                    </button>
                </div>

                    <button
                        onClick={startService}
                        className="w-full bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-400"
                    >
                        Start Dicom Service
                    </button>
                </div>
        </main>
);
}
