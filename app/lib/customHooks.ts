import {useEffect, useState} from "react";
import {load} from "@tauri-apps/plugin-store";

export const useSetupComplete = () => {
    const [setupComplete, setSetupComplete] = useState(false);
    const [isLoading, setIsLoading] = useState(true);

    useEffect(() => {
        // Async operations go inside useEffect
        async function fetchSetupStatus() {
            try {
                const store = await load('store.json', { autoSave: false } as any);
                const status = await store.get('setup_complete');
                setSetupComplete(status === true);
            } catch (error) {
                console.error('Error fetching setup status:', error);
            } finally {
                setIsLoading(false);
            }
        }

        fetchSetupStatus().then();

    }, []); // Empty dependency array means this runs once on mount

    return { setupComplete, isLoading };
};