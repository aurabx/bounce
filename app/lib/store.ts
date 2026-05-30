import { configureStore, createSlice } from '@reduxjs/toolkit'
import type { PayloadAction } from '@reduxjs/toolkit'
import {CurrentStudies, DicomService, Pagination, Study} from "@/app/lib/types";

export interface RunningDetail {
    label: string,
    value: string,
}

export interface LogEntry {
    id: string,
    level: 'trace' | 'debug' | 'info' | 'warn' | 'error',
    message: string,
    source: 'event' | 'system',
    timestamp: string,
}

export interface AppError {
    id: string,
    message: string,
}

const MAX_LOG_ENTRIES = 2000

export type ConnectivityStatus = 'idle' | 'checking' | 'ok' | 'failed';

export interface ConnectivityState {
    status: ConnectivityStatus,
    error: string | null,
    lastCheckedAt: number | null,
}

export interface State {
    logs: LogEntry[]
    studies: Study[],
    pagination: Pagination | null,
    running: boolean,
    runningDetail: RunningDetail[],
    errors: AppError[],
    dicomServices: DicomService[],
    connectivity: ConnectivityState,
}

const initialState: State = {
    logs: [],
    studies: [],
    pagination: null,
    running: false,
    runningDetail: [],
    errors: [],
    dicomServices: [],
    connectivity: {
        status: 'idle',
        error: null,
        lastCheckedAt: null,
    },
}

export const mainSlice = createSlice({
    name: 'main',
    initialState,
    reducers: {
        logMessage: (state, action: PayloadAction<LogEntry>) => {
            state.logs.push(action.payload)

            if (state.logs.length > MAX_LOG_ENTRIES) {
                state.logs.splice(0, state.logs.length - MAX_LOG_ENTRIES)
            }
        },
        clearLogs: (state) => {
            state.logs = []
        },
        setRunning(state, action: PayloadAction<boolean>){
            state.running = action.payload;
        },
        setRunningDetail(state, action: PayloadAction<string>){
            try {
                state.runningDetail = JSON.parse(action.payload) as RunningDetail[];
            } catch (e) {
                console.error('Failed to parse running detail payload', e);
            }
        },
        setCurrentStudies(state, action: PayloadAction<CurrentStudies>){
            state.studies = action.payload.studies
            state.pagination = action.payload.pagination ?? null
        },
        setError: {
            reducer(state, action: PayloadAction<AppError>){
                state.errors.push(action.payload)
            },
            prepare(message: string){
                return { payload: { id: crypto.randomUUID(), message } }
            },
        },
        setErrors(state, action: PayloadAction<AppError[]>){
            state.errors = action.payload
        },
        setDicomServices(state, action: PayloadAction<DicomService[]>){
            state.dicomServices = action.payload
        },
        setConnectivityChecking(state){
            state.connectivity.status = 'checking'
            state.connectivity.error = null
        },
        setConnectivityOk(state){
            state.connectivity.status = 'ok'
            state.connectivity.error = null
            state.connectivity.lastCheckedAt = Date.now()
        },
        setConnectivityFailed(state, action: PayloadAction<string>){
            state.connectivity.status = 'failed'
            state.connectivity.error = action.payload
            state.connectivity.lastCheckedAt = Date.now()
        },
    },
})

// Action creators are generated for each case reducer function
export const {
    logMessage,
    clearLogs,
    setRunning,
    setRunningDetail,
    setCurrentStudies,
    setError,
    setErrors,
    setDicomServices,
    setConnectivityChecking,
    setConnectivityOk,
    setConnectivityFailed,
} = mainSlice.actions

export const makeStore = () => {
    return configureStore({
        reducer: {
            main: mainSlice.reducer
        },
    })
}


// Infer the type of makeStore
export type AppStore = ReturnType<typeof makeStore>
// Infer the `RootState` and `AppDispatch` types from the store itself
export type RootState = ReturnType<AppStore['getState']>
export type AppDispatch = AppStore['dispatch']

// The function below is called a selector and allows us to select a value from
// the state. Selectors can also be defined inline where they're used instead of
// in the slice file. For example: `useSelector((state) => state.counter.value)`
export const selectLogs = (state: RootState) => state.main.logs

export const selectErrors = (state: RootState) => state.main.errors
export const selectRunning = (state: RootState) => state.main.running
