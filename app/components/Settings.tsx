'use client';

import {load} from '@tauri-apps/plugin-store';
import {Suspense, useEffect, useState, ChangeEvent, FormEvent, MouseEvent} from 'react'
import {fields, fieldKeys} from "@/app/lib/fields";
import SelectInput from "@/app/components/Fields/SelectInput";
import { Alert, AlertTitle, AlertDescription } from "@/app/components/ui/alert";
import { Button } from "@/app/components/ui/button";
import { Card, CardContent } from "@/app/components/ui/card";
import { open } from '@tauri-apps/plugin-dialog';
import { useRouter } from 'next/navigation'
import {useSetupComplete} from "@/app/lib/customHooks";
import {cn} from "@/app/lib/utils";
import {invokeCommand} from "@/app/lib/commands";
import { relaunch } from '@tauri-apps/plugin-process';
import { enable as enableAutostart, disable as disableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart';
import {useAppDispatch, useAppSelector} from "@/app/lib/hook";
import {verifyConnectivity} from "@/app/lib/server";
import {useUpdate} from "@/app/lib/UpdateContext";

const hourOptions: { [hour: string]: string } = Object.fromEntries(
    Array.from({ length: 24 }, (_, hour) => [
        String(hour),
        `${String(hour).padStart(2, '0')}:00`,
    ])
);

export default function Settings() {

    const [settings, setSettings] = useState<{ [key: string]: any }>({});
    const [loaded, setLoaded] = useState<boolean>(false);
    const [saved, setSaved] = useState<boolean>(false);
    const [env, setEnv] = useState<string|null>(null);
    const [autoUpdate, setAutoUpdate] = useState<string>('no');
    const [autoUpdateStart, setAutoUpdateStart] = useState<string>('0');
    const [autoUpdateEnd, setAutoUpdateEnd] = useState<string>('0');
    const [startOnLogin, setStartOnLogin] = useState<string>('no');
    const [autostartError, setAutostartError] = useState<string | null>(null);
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

    const save = async (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false });

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
        await invokeCommand('update_send_logs', { enabled: settings?.['send_logs'] === 'yes' });

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

    const setField = async (key: string, value: string) => {
        setSettings({
            ...settings,
            ...{[key]: value}
        })
    }

    const suffixClick = async (e: MouseEvent, key: string) => {
        e.preventDefault()
        if (key === 'base_dir') {
            const file = await open({
                multiple: false,
                directory: true,
            });
            if (typeof file === 'string') {
                await setField(key, file)
            }
        }
    }


    const loadStore = async () => {
        let store =  await load('store.json', { autoSave: false });

        let data: { [key: string]: any } = {}
        for (const fieldKey of fieldKeys) {
            let val = await store.get(fieldKey)
            if (val === null || val === undefined) {
                if (fieldKey === 'ae_title') val = 'BOUNCE';
                if (fieldKey === 'ip_address') val = '0.0.0.0';
                if (fieldKey === 'send_logs') val = 'yes';
                if (fieldKey === 'delete_after_success') val = 'yes';
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

        setAutoUpdate(((await store.get('auto_update')) as string) ?? 'no')
        setAutoUpdateStart(String((await store.get('auto_update_window_start')) ?? '0'))
        setAutoUpdateEnd(String((await store.get('auto_update_window_end')) ?? '0'))
    }

    const persistAutoUpdate = async (updates: {
        auto_update?: string,
        auto_update_window_start?: string,
        auto_update_window_end?: string,
    }) => {
        const store = await load('store.json', { autoSave: false });
        for (const [key, value] of Object.entries(updates)) {
            await store.set(key, value)
        }
        await store.save()
    }

    const resetApp = async (e: MouseEvent<HTMLButtonElement>) => {
        e.preventDefault();

        let store =  await load('store.json', { autoSave: false });
        await store.clear()
        await store.reset()

        try {
            await invokeCommand('reset_app'); // Pass the port to Tauri
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

    // The OS login-items mechanism is the source of truth for autostart, not
    // store.json. Read the real registered state so the toggle never drifts
    // from what the system will actually do on reboot.
    const toggleStartOnLogin = async (value: string) => {
        setAutostartError(null);
        const previous = startOnLogin;
        setStartOnLogin(value);
        try {
            if (value === 'yes') {
                await enableAutostart();
            } else {
                await disableAutostart();
            }
            const actual = await isAutostartEnabled();
            setStartOnLogin(actual ? 'yes' : 'no');
        } catch (error) {
            setStartOnLogin(previous);
            setAutostartError(String(error));
        }
    }

    useEffect(() => {
        isAutostartEnabled()
            .then((enabled) => setStartOnLogin(enabled ? 'yes' : 'no'))
            .catch((error) => setAutostartError(String(error)));
    }, [])

    useEffect(() => {
        loadStore().then(async () => {
            setLoaded(true)
            // Auto-verify connectivity on mount whenever a key is already
            // configured. With no key there is nothing meaningful to check
            // — leave the status as whatever it was (likely 'idle').
            const store = await load('store.json', { autoSave: false });
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
                                        const FieldComponent = field.component;
                                        const isFullWidth = field.config.fullWidth;
                                        return (
                                            <div key={field.config.key} className={isFullWidth ? "md:col-span-2" : ""}>
                                                <FieldComponent
                                                    config={field.config}
                                                    settings={settings}
                                                    onSuffixClick={field.config.suffix_button ? (e: MouseEvent) => suffixClick(e, field.config.key) : () => null}
                                                    value={settings && settings[field.config.key] ? settings[field.config.key] : ''}
                                                    onChange={(e: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => setField(field.config.key, e.target.value)}
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
                        <h3 className="text-base font-medium">Startup</h3>
                        <p className="text-sm text-muted-foreground">
                            Start Bounce automatically when you log in so the
                            receiver comes back up after a reboot.
                        </p>
                    </div>

                    <div className="space-y-4 border-t pt-4">
                        <SelectInput
                            config={{
                                label: 'Start on login',
                                key: 'start_on_login',
                                help: 'Launch Bounce automatically when this user signs in to the computer.',
                                options: { no: 'No', yes: 'Yes' },
                            }}
                            value={startOnLogin}
                            onChange={async (e: any) => {
                                await toggleStartOnLogin(e.target.value);
                            }}
                        />

                        {autostartError && (
                            <Alert variant="destructive">
                                <AlertTitle>Could not change startup setting</AlertTitle>
                                <AlertDescription>{autostartError}</AlertDescription>
                            </Alert>
                        )}
                    </div>
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

                    <div className="space-y-4 border-t pt-4">
                        <SelectInput
                            config={{
                                label: 'Automatic updates',
                                key: 'auto_update',
                                help: 'Automatically restart Bounce to apply downloaded updates. If the receiver is running, it is restarted running.',
                                options: { no: 'No', yes: 'Yes' },
                            }}
                            value={autoUpdate}
                            onChange={async (e: any) => {
                                const value = e.target.value;
                                setAutoUpdate(value);
                                await persistAutoUpdate({ auto_update: value });
                            }}
                        />

                        {autoUpdate === 'yes' && (
                            <div className="grid grid-cols-1 gap-x-8 gap-y-6 md:grid-cols-2">
                                <SelectInput
                                    config={{
                                        label: 'Restart window start',
                                        key: 'auto_update_window_start',
                                        help: 'Earliest local time Bounce may restart to apply an update.',
                                        options: hourOptions,
                                    }}
                                    value={autoUpdateStart}
                                    onChange={async (e: any) => {
                                        const value = e.target.value;
                                        setAutoUpdateStart(value);
                                        await persistAutoUpdate({ auto_update_window_start: value });
                                    }}
                                />
                                <SelectInput
                                    config={{
                                        label: 'Restart window end',
                                        key: 'auto_update_window_end',
                                        help: 'Latest local time Bounce may restart. Set start and end to the same time to allow restarts at any time.',
                                        options: hourOptions,
                                    }}
                                    value={autoUpdateEnd}
                                    onChange={async (e: any) => {
                                        const value = e.target.value;
                                        setAutoUpdateEnd(value);
                                        await persistAutoUpdate({ auto_update_window_end: value });
                                    }}
                                />
                            </div>
                        )}
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