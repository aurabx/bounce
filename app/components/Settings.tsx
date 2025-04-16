'use client';

import {load} from '@tauri-apps/plugin-store';
import {Suspense, useEffect, useState} from 'react'
import {fields, fieldKeys} from "@/app/lib/fields";
import Alert from "@/app/components/Fields/Alert";
import { open } from '@tauri-apps/plugin-dialog';
import { useRouter } from 'next/navigation'
import {useSetupComplete} from "@/app/lib/customHooks";
import {classNames} from "@/app/lib/helpers";
import SelectInput from "@/app/components/Fields/SelectInput";
import { relaunch } from "@tauri-apps/plugin-process";

export default function Settings() {

    const [settings, setSettings] = useState<{ [key: string]: any }>({});
    const [loaded, setLoaded] = useState<boolean>(false);
    const [saved, setSaved] = useState<boolean>(false);
    const [isSuper, setIsSuper] = useState<boolean>(false);
    const router = useRouter();
    const { setupComplete } = useSetupComplete();

    const save = async (e: any) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false });

        for (const fieldKey of fieldKeys) {
            console.log(fieldKey, settings?.[fieldKey])
            await store.set(fieldKey, settings?.[fieldKey] ?? '')
        }

        if (await store.get('api_key') && await store.get('port') && await store.get('base_dir')){
            await store.set('setup_complete', true);
        }

        await store.save();

        setSaved(true);
        setTimeout(() => setSaved(false), 3000)

        await checkApiKey()

        if (!setupComplete){
            router.push('/')
            window.location.reload()
        }
    };

    const checkApiKey = async () => {
        if (settings && settings.api_key) {
            const parts = settings.api_key.split('_');
            const last = parts[parts.length - 1];

            ['local', 'dev', 'staging'].includes(last) ? setIsSuper(true) : setIsSuper(false);
        }
    }

    const setField = async (key: string, value: any) => {
        setSettings({
            ...settings,
            ...{[key]: value}
        })
    }

    const suffixClick = async (e: any, key: string) => {
        e.preventDefault()
        if (key === 'base_dir') {
            const file = await open({
                multiple: false,
                directory: true,
            });
            await setField(key, file)
        }
    }


    const loadStore = async () => {
        let store =  await load('store.json', { autoSave: false });
        let values =  await store.entries();

        let data: { [key: string]: any } = {}
        for (const fieldKey of fieldKeys) {
            data[fieldKey] = await store.get(fieldKey)
        }
        setSettings(data)
    }

    const resetApp = async (e: any) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false });
        await store.clear()
        await store.reset()

        router.push('/')
        window.location.reload();
    }

    const modeConfig = {
        label: 'Mode',
        key: 'mode',
        help: 'Select the mode for bounce to run in',
        options: {
            production: "Production",
            staging: "Staging",
            development: "Development",
            local: "Local",
        },
    }

    useEffect(() => {
        loadStore().then(async () => {
            setLoaded(true)
        })
    }, [])

    useEffect(() => {
        checkApiKey().then()
    }, [settings, isSuper]);

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
                                        onSuffixClick={field.config.suffix_button ? (e: MouseEvent) => suffixClick(e, field.config.key) : () => null}
                                        value={settings && settings[field.config.key] ? settings[field.config.key] : ''}
                                        onChange={(e: any) => setField(field.config.key, e.target.value)}/>)
                                })}

                                {isSuper && (
                                    <SelectInput
                                        key="mode"
                                        settings={settings}
                                        value={settings && settings.mode ? settings.mode : ''}
                                        onChange={(e: any) => setField('mode', e.target.value)}
                                        config={modeConfig}
                                    />
                                )}

                            </div>
                            <div className={classNames("mb-4 flex", setupComplete ? 'justify-between' : 'justify-end')}>
                                {setupComplete && (
                                    <button
                                        type="button"
                                        onClick={resetApp}
                                        className="inline-flex grow-0 transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer border-slate-400 text-slate-600 bg-white hover:bg-slate-100"
                                    >
                                        Reset App
                                    </button>
                                )}
                                <button
                                    type="submit"
                                    className="inline-flex grow-0 transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer border-indigo-400 text-white bg-indigo-400 hover:bg-indigo-500"
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