'use client';

import {
    createContext,
    ReactNode,
    useCallback,
    useContext,
    useEffect,
    useRef,
    useState,
} from 'react';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { load } from '@tauri-apps/plugin-store';
import { useAppSelector } from '@/app/lib/hook';

export type UpdateStatus =
    | 'idle'
    | 'checking'
    | 'downloading'
    | 'ready'
    | 'up-to-date'
    | 'error';

export interface UpdateInfo {
    version: string;
    notes?: string;
}

interface UpdateContextValue {
    status: UpdateStatus;
    updateInfo: UpdateInfo | null;
    errorMessage: string;
    checkAndDownload: () => Promise<void>;
    restartApp: () => void;
}

const CHECK_INTERVAL_MS = 60 * 60 * 1000; // 1 hour
const WINDOW_POLL_INTERVAL_MS = 60 * 1000; // re-check the restart window each minute

interface AutoUpdatePrefs {
    enabled: boolean;
    startHour: number;
    endHour: number;
}

/**
 * Read the auto-update preferences from the persisted settings store.
 * Returns disabled defaults if the store cannot be read.
 */
async function loadAutoUpdatePrefs(): Promise<AutoUpdatePrefs> {
    try {
        const store = await load('store.json', { autoSave: false } as any);
        const enabled = (await store.get('auto_update')) === 'yes';

        const parseHour = (value: unknown, fallback: number): number => {
            const parsed = parseInt(String(value ?? ''), 10);
            return Number.isFinite(parsed) && parsed >= 0 && parsed <= 23 ? parsed : fallback;
        };

        return {
            enabled,
            startHour: parseHour(await store.get('auto_update_window_start'), 0),
            endHour: parseHour(await store.get('auto_update_window_end'), 0),
        };
    } catch {
        return { enabled: false, startHour: 0, endHour: 0 };
    }
}

/**
 * Decide whether the current local time falls inside the configured restart
 * window. A window where start equals end is treated as "any time". Windows
 * where the start hour is greater than the end hour wrap past midnight.
 */
function isWithinWindow(startHour: number, endHour: number, now: Date): boolean {
    if (startHour === endHour) return true;
    const hour = now.getHours();
    if (startHour < endHour) return hour >= startHour && hour < endHour;
    return hour >= startHour || hour < endHour;
}

const UpdateContext = createContext<UpdateContextValue | null>(null);

export function useUpdate(): UpdateContextValue {
    const ctx = useContext(UpdateContext);
    if (!ctx) throw new Error('useUpdate must be used inside <UpdateProvider>');
    return ctx;
}

export function UpdateProvider({ children }: { children: ReactNode }) {
    const [status, setStatus] = useState<UpdateStatus>('idle');
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [errorMessage, setErrorMessage] = useState('');

    const pendingUpdateRef = useRef<Update | null>(null);
    const checkInProgressRef = useRef(false);
    const lastCheckTimeRef = useRef<number>(0);

    // Mirror the receiver's running state into a ref so the relaunch path can
    // read the latest value without re-creating its callbacks on every change.
    const running = useAppSelector((state) => state.main.running);
    const runningRef = useRef(running);
    useEffect(() => {
        runningRef.current = running;
    }, [running]);

    // Persist whether the receiver is currently running, then relaunch. On the
    // next boot EventHandler reads this flag and restarts the receiver so the
    // service comes back in the same state it was in before the restart.
    const persistRunningStateAndRelaunch = useCallback(async () => {
        try {
            const store = await load('store.json', { autoSave: false } as any);
            await store.set('_resume_running', runningRef.current);
            await store.save();
        } catch (e) {
            // If the flag cannot be persisted we still relaunch; the only
            // consequence is the operator may need to start the service again.
            console.error('Failed to persist resume-running flag', e);
        }
        await relaunch();
    }, []);

    const checkAndDownload = useCallback(async () => {
        if (checkInProgressRef.current) return;
        checkInProgressRef.current = true;

        setStatus('checking');
        setErrorMessage('');

        try {
            const update = await check();

            if (!update) {
                setStatus('up-to-date');
                return;
            }

            pendingUpdateRef.current = null;
            setUpdateInfo({ version: update.version, notes: update.body ?? undefined });

            setStatus('downloading');
            await update.downloadAndInstall();

            pendingUpdateRef.current = update;
            setStatus('ready');
        } catch (e) {
            setErrorMessage(String(e));
            setStatus('error');
        } finally {
            checkInProgressRef.current = false;
        }
    }, []);

    const restartApp = useCallback(() => {
        void persistRunningStateAndRelaunch();
    }, [persistRunningStateAndRelaunch]);

    // Once an update has been downloaded and installed, automatically relaunch
    // when auto-update is enabled and the current time is inside the configured
    // restart window. Outside the window we poll each minute and relaunch as
    // soon as the window opens.
    useEffect(() => {
        if (status !== 'ready') return;

        let cancelled = false;
        let timer: ReturnType<typeof setInterval> | null = null;

        const attemptAutoRestart = async () => {
            if (cancelled) return;
            const prefs = await loadAutoUpdatePrefs();
            if (!prefs.enabled) return;
            if (isWithinWindow(prefs.startHour, prefs.endHour, new Date())) {
                if (timer) clearInterval(timer);
                await persistRunningStateAndRelaunch();
            }
        };

        void attemptAutoRestart();
        timer = setInterval(() => {
            void attemptAutoRestart();
        }, WINDOW_POLL_INTERVAL_MS);

        return () => {
            cancelled = true;
            if (timer) clearInterval(timer);
        };
    }, [status, persistRunningStateAndRelaunch]);

    useEffect(() => {
        const initialTimer = setTimeout(() => {
            checkAndDownload();
        }, 5_000);

        const interval = setInterval(() => {
            if (document.visibilityState === 'visible') {
                checkAndDownload();
            }
        }, CHECK_INTERVAL_MS);

        return () => {
            clearTimeout(initialTimer);
            clearInterval(interval);
        };
    }, [checkAndDownload]);

    useEffect(() => {
        function handleVisibilityChange() {
            if (document.visibilityState !== 'visible') return;
            if (checkInProgressRef.current) return;
            if (Date.now() - lastCheckTimeRef.current < CHECK_INTERVAL_MS) return;
            checkAndDownload();
        }

        document.addEventListener('visibilitychange', handleVisibilityChange);
        return () =>
            document.removeEventListener('visibilitychange', handleVisibilityChange);
    }, [checkAndDownload]);

    useEffect(() => {
        if (status === 'up-to-date' || status === 'ready' || status === 'error') {
            lastCheckTimeRef.current = Date.now();
        }
    }, [status]);

    return (
        <UpdateContext.Provider
            value={{ status, updateInfo, errorMessage, checkAndDownload, restartApp }}
        >
            {children}
        </UpdateContext.Provider>
    );
}
