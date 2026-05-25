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
        relaunch();
    }, []);

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
