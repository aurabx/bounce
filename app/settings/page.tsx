'use client';

import { load } from '@tauri-apps/plugin-store';
import {useEffect, useState} from 'react'
import fields from "@/app/lib/fields";

export default function Page() {

    const [settings, setSettings] = useState<{ [key: string]: any }>();

    const save = async () => {
        let store =  await load('store.json', { autoSave: false });

        for (const key in settings) {
            await store.set(key, settings[key])
        }

        await store.save();
    };

    const setField = (key: string, value: any) => {
        console.log({
            [key]: value
        })

        setSettings({
            ...settings,
            [key]: value
        })
    }

    useEffect(() => {
        const loadStore = async () => {
            let store =  await load('store.json', { autoSave: false });
            let values =  await store.entries();

            console.log('values', values)

            for (const field of fields) {
                const value = await store.get(field.config.key)
                setField(field.config.key, value)
            }
        }

        loadStore().finally()
    }, [])

    return (
        <main >
            <h1 className="text-3xl font-bold mb-6">
                Settings
            </h1>
            <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-6 w-full max-w-xl">
                <form>
                    <div className="space-y-12">
                        <div
                            className="grid grid-cols-1 gap-x-8 gap-y-6 md:grid-cols-3">
                            {fields.map((field) => {
                                const FieldComponent: any = field.component;

                                return (<FieldComponent
                                    key={field.config.key}
                                    config={field.config}
                                    settings={settings}
                                    onChange={(e: any) => setField(field.config.key, e.target.value)}/>)
                            })}
                            <div className="mb-4">
                                <button
                                    onClick={save}
                                    className="w-full bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-400"
                                >
                                    Save
                                </button>
                            </div>
                        </div>
                    </div>
                </form>
            </div>
        </main>
);
}
