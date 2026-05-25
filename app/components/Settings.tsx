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
import {useAppDispatch, useAppSelector} from "@/app/lib/hook";
import {verifyConnectivity} from "@/app/lib/server";
import {useUpdate} from "@/app/lib/UpdateContext";

export default function Settings() {

    const [settings, setSettings] = useState<{ [key: string]: any }>({});
    const [loaded, setLoaded] = useState<boolean>(false);
    const [saved, setSaved] = useState<boolean>(false);
    const [env, setEnv] = useState<string|null>(null);
    const router = useRouter();
    const { setupComplete } = useSetupComplete();
    const dispatch = useAppDispatch();
    const connectivity = useAppSelector((state) => state.main.connectivity);
    const verifying = connectivity.status === 'checking';
    const verifyError = connectivity.status === 'failed' ? connectivity.error : null;
    const {
        status: updateStatus,
        updateInfo,
        errorMessage: updateError,
        checkAndDownload,
        restartApp,
    } = useUpdate();

    const deriveEnv = (apiKey: string | null | undefined): string | null => {
        if (!apiKey) return null;
        const parts = apiKey.split('_');
        const last = parts[parts.length - 1];
        return ['local', 'dev', 'staging'].includes(last) ? last : null;
    };

    const save = async (e: any) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false } as any);

        for (const fieldKey of fieldKeys) {
            // Internal-only flags (prefixed with _) are not persisted.
            if (fieldKey === 'api_key') {
                // Only overwrite the stored API key when the user has typed a
                // new value. An empty input means "no change" — the previously
                // saved key stays intact and unrevealed.
                const next = (settings?.api_key ?? '').toString().trim();
                if (next.length > 0) {
                    await store.set('api_key', next);
                }
                continue;
            }
            await store.set(fieldKey, settings?.[fieldKey] ?? '')
        }

        if (await store.get('api_key') && await store.get('port') && await store.get('base_dir')){
            await store.set('setup_complete', true);
        }

        await store.save();

        // Update remote logging flag in the backend without requiring a restart
        await invoke('update_send_logs', { enabled: settings?.['send_logs'] === 'yes' });

        // Update env warning from whatever key is now stored (may be the new
        // one or the existing one, depending on whether the user changed it).
        const persistedKey = (await store.get('api_key')) as string | null;
        setEnv(deriveEnv(persistedKey));

        // Reset the form's API key state: hide any value the user just typed
        // and re-enter masked-display mode now that a key is saved.
        setSettings((prev) => ({
            ...prev,
            api_key: '',
            _has_api_key: !!persistedKey && persistedKey.length > 0,
        }));

        // Verify connectivity using the now-persisted key. Result lands in
        // Redux so the dashboard's status card updates too.
        const verifyOk = await verifyConnectivity(dispatch);
        if (verifyOk) {
            setSaved(true);
            setTimeout(() => setSaved(false), 3000)
        }

        if (!setupComplete && verifyOk){
            router.push('/')
            window.location.reload()
        }
    };

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
        let store =  await load('store.json', { autoSave: false } as any);

        let data: { [key: string]: any } = {}
        for (const fieldKey of fieldKeys) {
            let val = await store.get(fieldKey)
            if (val === null || val === undefined) {
                if (fieldKey === 'ae_title') val = 'BOUNCE';
                if (fieldKey === 'ip_address') val = '0.0.0.0';
                if (fieldKey === 'send_logs') val = 'yes';
            }

            // The API key is never copied into form state. We expose only
            // a boolean indicating whether one is currently saved, plus a
            // pre-derived environment label for the warning banner. This
            // makes it impossible to read the saved key out of the UI.
            if (fieldKey === 'api_key') {
                const existing = typeof val === 'string' ? val : '';
                data['api_key'] = '';
                data['_has_api_key'] = existing.length > 0;
                setEnv(deriveEnv(existing));
                continue;
            }

            data[fieldKey] = val
        }
        setSettings(data)
    }

    const resetApp = async (e: any) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false } as any);
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
            // Auto-verify connectivity on mount whenever a key is already
            // configured. With no key there is nothing meaningful to check
            // — leave the status as whatever it was (likely 'idle').
            const store = await load('store.json', { autoSave: false } as any);
            const existingKey = (await store.get('api_key')) as string | null;
            if (existingKey && existingKey.length > 0) {
                await verifyConnectivity(dispatch);
            }
        })
    }, [dispatch])

    // When the user types a new key in the form, reflect the implied env in
    // the warning banner immediately. We deliberately do not derive env from
    // a saved key here — loadStore() already did that on mount.
    useEffect(() => {
        const typed = (settings?.api_key ?? '') as string;
        if (typed.length > 0) {
            setEnv(deriveEnv(typed));
        }
    }, [settings?.api_key]);

    return (
        <div className="space-y-4">
            {saved && <Alert variant="default" className="bg-green-50 text-green-800 border-green-200">
                <AlertTitle>Success</AlertTitle>
                <AlertDescription>Settings saved and connectivity verified.</AlertDescription>
            </Alert>}
            {verifyError && <Alert variant="destructive">
                <AlertTitle>Connectivity check failed</AlertTitle>
                <AlertDescription>
                    Settings were saved, but Bounce could not reach Aurabox with the configured API key. {verifyError}
                </AlertDescription>
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
                                        disabled={verifying}
                                    >
                                        {verifying ? 'Verifying…' : 'Save'}
                                    </Button>
                                </div>
                            </div>
                        </form> : null)}
                    </Suspense>
                </CardContent>
            </Card>

            <Card>
                <CardContent className="pt-6 space-y-3">
                    <div>
                        <h3 className="text-base font-medium">Updates</h3>
                        <p className="text-sm text-muted-foreground">
                            Bounce checks GitHub for new releases automatically and
                            downloads them in the background.
                        </p>
                    </div>

                    {updateStatus === 'up-to-date' && (
                        <p className="text-sm text-muted-foreground">
                            You&apos;re on the latest version.
                        </p>
                    )}

                    {updateStatus === 'downloading' && (
                        <p className="text-sm">
                            {updateInfo
                                ? `Downloading ${updateInfo.version}…`
                                : 'Downloading update…'}
                        </p>
                    )}

                    {updateStatus === 'ready' && (
                        <div className="space-y-2">
                            <p className="text-sm">
                                Update installed{updateInfo ? ` — v${updateInfo.version}` : ''}.
                                Restart Bounce to apply.
                            </p>
                            {updateInfo?.notes && (
                                <pre className="text-xs bg-muted/40 rounded p-2 whitespace-pre-wrap">
                                    {updateInfo.notes}
                                </pre>
                            )}
                        </div>
                    )}

                    {updateStatus === 'error' && updateError && (
                        <Alert variant="destructive">
                            <AlertTitle>Update failed</AlertTitle>
                            <AlertDescription>{updateError}</AlertDescription>
                        </Alert>
                    )}

                    <div className="flex gap-2">
                        {updateStatus !== 'ready' && (
                            <Button
                                type="button"
                                variant="outline"
                                onClick={checkAndDownload}
                                disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
                            >
                                {updateStatus === 'checking'
                                    ? 'Checking…'
                                    : updateStatus === 'downloading'
                                        ? 'Downloading…'
                                        : 'Check for Updates'}
                            </Button>
                        )}
                        {updateStatus === 'ready' && (
                            <Button type="button" onClick={restartApp}>
                                Restart Now
                            </Button>
                        )}
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

function Loading() {
    return <h2>🌀 Loading...</h2>;
}