'use client';

import {load} from '@tauri-apps/plugin-store';
import {Suspense, useEffect, useState} from 'react'
import {fields, fieldKeys} from "@/app/lib/fields";
import { Alert, AlertTitle, AlertDescription } from "@/app/components/ui/alert";
import { Button } from "@/app/components/ui/button";
import { Card, CardContent } from "@/app/components/ui/card";
import { open } from '@tauri-apps/plugin-dialog';
import { useRouter } from 'next/navigation'
import {useSetupComplete} from "@/app/lib/customHooks";
import {cn} from "@/app/lib/utils";
import {invoke} from "@tauri-apps/api/core";
import { relaunch } from '@tauri-apps/plugin-process';

export default function Settings() {

    const [settings, setSettings] = useState<{ [key: string]: any }>({});
    const [loaded, setLoaded] = useState<boolean>(false);
    const [saved, setSaved] = useState<boolean>(false);
    const [env, setEnv] = useState<string|null>(null);
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

            ['local', 'dev', 'staging'].includes(last) ? setEnv(last) : setEnv(null);
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
            let val = await store.get(fieldKey)
            if (val === null || val === undefined) {
                if (fieldKey === 'ae_title') val = 'BOUNCE';
                if (fieldKey === 'ip_address') val = '0.0.0.0';
                if (fieldKey === 'send_logs') val = 'yes';
            }
            data[fieldKey] = val
        }
        setSettings(data)
    }

    const resetApp = async (e: any) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false });
        await store.clear()
        await store.reset()

        try {
            await invoke('reset_app'); // Pass the port to Tauri
        } catch (error) {
            console.error('Error resetting app:', error);
            alert(`Error resetting app: ${error}`);
        }

        await relaunch()

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
    }, [settings, env]);

    return (
        <div className="space-y-4">
            {saved && <Alert variant="default" className="bg-green-50 text-green-800 border-green-200">
                <AlertTitle>Success</AlertTitle>
                <AlertDescription>Settings saved successfully.</AlertDescription>
            </Alert>}
            {env && <Alert variant="destructive">
                <AlertTitle>Environment Warning</AlertTitle>
                <AlertDescription>You are connected to the {env} environment.</AlertDescription>
            </Alert>}

            <Card>
                <CardContent className="pt-6">
                    <Suspense fallback={<Loading />}>
                        {(loaded ? <form onSubmit={save}>
                            <div className="space-y-8">
                                <div className="grid grid-cols-1 gap-x-8 gap-y-6 md:grid-cols-2">
                                    {fields.map((field) => {
                                        const FieldComponent: any = field.component;
                                        // @ts-ignore
                                        const isFullWidth = field.config.fullWidth;
                                        return (
                                            <div key={field.config.key} className={isFullWidth ? "md:col-span-2" : ""}>
                                                <FieldComponent
                                                    config={field.config}
                                                    settings={settings}
                                                    onSuffixClick={field.config.suffix_button ? (e: MouseEvent) => suffixClick(e, field.config.key) : () => null}
                                                    value={settings && settings[field.config.key] ? settings[field.config.key] : ''}
                                                    onChange={(e: any) => setField(field.config.key, e.target.value)}
                                                />
                                            </div>
                                        )
                                    })}
                                </div>
                                <div className={cn("flex", setupComplete ? 'justify-between' : 'justify-end')}>
                                    {setupComplete && (
                                        <Button
                                            type="button"
                                            variant="outline"
                                            onClick={resetApp}
                                        >
                                            Reset App
                                        </Button>
                                    )}
                                    <Button
                                        type="submit"
                                    >
                                        Save
                                    </Button>
                                </div>
                            </div>
                        </form> : null)}
                    </Suspense>
                </CardContent>
            </Card>
        </div>
    );
}

function Loading() {
    return <h2>🌀 Loading...</h2>;
}