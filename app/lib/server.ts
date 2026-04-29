import {load} from "@tauri-apps/plugin-store";
import {invoke} from "@tauri-apps/api/core";
import type {AppDispatch} from "@/app/lib/store";
import {
    setConnectivityChecking,
    setConnectivityFailed,
    setConnectivityOk,
} from "@/app/lib/store";

/**
 * Run the backend connectivity check and reflect the result into Redux so
 * any component (Dashboard status card, Settings page, etc.) sees a single
 * source of truth.
 *
 * Returns true on success, false on failure. Never throws.
 */
export const verifyConnectivity = async (dispatch: AppDispatch): Promise<boolean> => {
    dispatch(setConnectivityChecking());
    try {
        await invoke('verify_connectivity');
        dispatch(setConnectivityOk());
        return true;
    } catch (err) {
        const message = typeof err === 'string'
            ? err
            : (err instanceof Error ? err.message : 'Connectivity check failed.');
        dispatch(setConnectivityFailed(message));
        return false;
    }
};

export const receiverStart = async () => {
    try {
        await invoke('receiver_start'); // Pass the port to Tauri
    } catch (error) {
        console.error('Error starting server:', error);
        alert(`Failed to start server: ${error}`);
    }
};

export const receiverStop = async () => {
    try {
        await invoke('receiver_stop'); // Pass the port to Tauri
        console.info(`Dicom server stopped`);
    } catch (error) {
        console.error('Error stopping server:', error);
        alert(`Failed to stopping server: ${error}`);
    }
};