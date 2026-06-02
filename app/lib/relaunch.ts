import { load } from '@tauri-apps/plugin-store';
import { relaunch as tauriRelaunch } from '@tauri-apps/plugin-process';

/**
 * Persist whether the DICOM receiver is currently running, then relaunch
 * the app. On the next launch, `EventHandler` reads the `_resume_running`
 * flag from `store.json` and starts the receiver again so the service
 * comes back in the same state it was in before the restart.
 *
 * All relaunch paths that should preserve receiver state (Settings →
 * Restart Now, automatic-update restart, tray → Relaunch) must go
 * through this function rather than calling `relaunch` directly.
 */
export async function persistRunningStateAndRelaunch(running: boolean): Promise<void> {
    try {
        const store = await load('store.json', { autoSave: false, defaults: {} });
        await store.set('_resume_running', running);
        await store.save();
    } catch (e) {
        // If the flag cannot be persisted we still relaunch; the only
        // consequence is the operator may need to start the service again.
        console.error('Failed to persist resume-running flag', e);
    }
    await tauriRelaunch();
}
