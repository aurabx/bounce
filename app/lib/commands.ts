import { invoke, InvokeArgs } from '@tauri-apps/api/core'
import { DicomService, EchoResult } from './types'

/**
 * Typed contract for the Rust Tauri commands exposed by the backend.
 *
 * Each entry mirrors a `#[tauri::command]` in `src-tauri/src/main.rs`: `args`
 * is the camelCase argument object Tauri expects (or `undefined` when the
 * command takes none), and `result` is the resolved value. Commands that emit
 * a Tauri event rather than returning data resolve to `void`.
 *
 * Keeping this in one place means a backend signature change surfaces as a
 * TypeScript error at the call site instead of a silent runtime failure.
 */
export interface CommandSignatures {
    send_log: { args: { log: string }; result: void }
    update_send_logs: { args: { enabled: boolean }; result: void }
    verify_connectivity: { args?: undefined; result: string }
    reset_app: { args?: undefined; result: void }
    api_start_upload: {
        args: { studyUid: string; signature: string; uploadId: string; assemblyId: string }
        result: void
    }
    send_study: { args: { studyUid: string }; result: void }
    retry_study: { args: { studyUid: string }; result: void }
    study_upload_attempts: { args: { studyUid: string }; result: unknown }
    delete_study: { args: { studyUid: string }; result: void }
    receiver_start: { args?: undefined; result: void }
    receiver_stop: { args?: undefined; result: void }
    // current_studies emits a "current-studies" event; it returns nothing.
    current_studies: { args?: { page?: number; limit?: number; search?: string }; result: void }
    bulk_send_studies: { args: { studyUids: string[] }; result: void }
    bulk_delete_studies: { args: { studyUids: string[] }; result: void }
    delete_all_studies: { args?: undefined; result: void }
    cfind_query: {
        args: {
            pacsHost: string
            pacsPort: number
            pacsAeTitle: string
            patientName?: string
            patientId?: string
            studyDate?: string
            accessionNumber?: string
            modality?: string
        }
        result: unknown[]
    }
    list_pacs_services: { args?: undefined; result: DicomService[] }
    refresh_pacs_services: { args?: undefined; result: DicomService[] }
    echo_pacs_service: { args: { serviceId: string }; result: EchoResult }
    show_window: { args?: undefined; result: void }
}

type CommandName = keyof CommandSignatures
type CommandArgs<K extends CommandName> = CommandSignatures[K]['args']
type CommandResult<K extends CommandName> = CommandSignatures[K]['result']

/**
 * Invoke a backend Tauri command with compile-time-checked arguments and
 * return type. Commands declared without `args` take no second parameter;
 * commands that declare `args` require it.
 */
export function invokeCommand<K extends CommandName>(
    command: K,
    ...rest: CommandArgs<K> extends undefined
        ? []
        : undefined extends CommandArgs<K>
          ? [args?: CommandArgs<K>]
          : [args: CommandArgs<K>]
): Promise<CommandResult<K>> {
    return invoke<CommandResult<K>>(command, rest[0] as InvokeArgs | undefined)
}
