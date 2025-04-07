import {load} from "@tauri-apps/plugin-store";
import {invoke} from "@tauri-apps/api/core";


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