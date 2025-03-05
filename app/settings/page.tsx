'use client';

import {load} from '@tauri-apps/plugin-store';
import {Suspense, useEffect, useState} from 'react'
import fields from "@/app/lib/fields";
import Alert from "@/app/components/Fields/Alert";

export default function Page() {

    const [settings, setSettings] = useState<{ [key: string]: any }>();
    const [loaded, setLoaded] = useState<boolean>(false);
    const [saved, setSaved] = useState<boolean>(false);

    const save = async (e: any) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false });

        for (const field of fields) {
            await store.set(field.config.key, settings?.[field.config.key] ? settings?.[field.config.key] : null)
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
        <>
            {(saved ? <Alert>Saved</Alert> : null)}
            <div className="bg-white shadow-lg rounded-lg p-6 w-full">
                <Suspense fallback={<Loading />}>
                    {(loaded ? <form onSubmit={save}>
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
                            </div>
                            <div className="mb-4 flex justify-end">
                                <button
                                    type="submit"
                                    className="inline-flex grow-0 transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                                >
                                    Save
                                </button>
                            </div>
                        </div>
                    </form> : null)}
                </Suspense>
            </div>
        </>
    );
}

function Loading() {
    return <h2>🌀 Loading...</h2>;
}