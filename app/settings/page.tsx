'use client';

import { load } from '@tauri-apps/plugin-store';
import {useEffect, useState} from 'react'

export default function Page() {

    let store;
    const [apiKey, setApikey] = useState<string>('');

    const save = async () => {

    };

    useEffect(() => {
        const loadStore = async () => {
            let store =  await load('store.json', { autoSave: false });
            let values =  await store.entries();

            // console.log()

            for (const key of ['api_key']) {
                const value = await store.get(key)
                setApikey(value as string)
            }


        }

        loadStore()
    }, [])

    return (
        <main className="flex min-h-screen flex-col items-center justify-center p-6">
            <h1 className="text-3xl font-bold mb-6 text-center">
                Aurabox Proxy TCP Server
            </h1>
            <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-6 w-full max-w-md">
                <div className="mb-4">
                    <label htmlFor="api_key" className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                        Api key
                    </label>
                    <input
                        type="text"
                        id="api_key"
                        value={apiKey}
                        onChange={(e) => setApikey(e.target.value)}
                        className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                        placeholder="Enter api key"
                    />
                </div>
                <div className="mb-4">
                    <button
                        onClick={save}
                        className="w-full bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-400"
                    >
                        Save
                    </button>
                </div>
            </div>
        </main>
);
}
