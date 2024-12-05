'use client';

import {load} from '@tauri-apps/plugin-store';
import {Suspense, useEffect, useState} from 'react'
import fields from "@/app/lib/fields";
import Alert from "@/app/components/Fields/Alert";

export default function Page() {

    const [settings, setSettings] = useState<{ [key: string]: any }>();
    const [loaded, setLoaded] = useState<boolean>(false);
    const [saved, setSaved] = useState<boolean>(false);

    const save = async () => {
        let store =  await load('store.json', { autoSave: false });

        for (const field of fields) {
            await store.set(field.config.key, settings ? settings[field.config.key] : null)
        }

        await store.save();

        setSaved(true);
        setTimeout(() => setSaved(false), 3000)
    };

    const setField = async (key: string, value: any) => {
        setSettings({
            ...settings,
            ...{[key]: value}
        })
    }

    useEffect(() => {
        const loadStore = async () => {
            let store =  await load('store.json', { autoSave: false });
            let values =  await store.entries();

            let data: { [key: string]: any } = {}
            for (const field of fields) {
                data[field.config.key] = await store.get(field.config.key)
            }
            setSettings(data)
        }

        loadStore().then(() => {
          setLoaded(true)
        })
    }, [])

    return (
        <main >
            <h1 className="text-3xl font-bold mb-6">
                Settings
            </h1>
            {(saved ? <Alert>Saved</Alert> : null)}
            <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-6 w-full max-w-xl">
                <Suspense fallback={<Loading />}>
                    {(loaded ? <form>
                        <div className="space-y-12">
                            <div
                                className="grid grid-cols-1 gap-x-8 gap-y-6 md:grid-cols-3">
                                {fields.map((field) => {
                                    const FieldComponent: any = field.component;

                                    return (<FieldComponent
                                        key={field.config.key}
                                        config={field.config}
                                        settings={settings}
                                        value={settings && settings[field.config.key] ? settings[field.config.key] : undefined}
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
                    </form> : null)}
                </Suspense>
            </div>
        </main>
    );
}

function Loading() {
    return <h2>🌀 Loading...</h2>;
}