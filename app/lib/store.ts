import { configureStore, createSlice } from '@reduxjs/toolkit'
import type { PayloadAction } from '@reduxjs/toolkit'
import {CurrentStudies, Study} from "@/app/lib/types";

export interface RunningDetail {
    label: string,
    value: string,
}

export interface State {
    logs: string[]
    studies: Study[],
    running: boolean,
    runningDetail: RunningDetail[],
}

const initialState: State = {
    logs: [],
    studies: [],
    running: false,
    runningDetail: [],
}

export const mainSlice = createSlice({
    name: 'main',
    initialState,
    reducers: {
        logMessage: (state, action: PayloadAction<string>) => {
            state.logs.push(action.payload)
        },
        setRunning(state, action: PayloadAction<boolean>){
            state.running = action.payload;
        },
        setRunningDetail(state, action: PayloadAction<string>){
            state.runningDetail = JSON.parse(action.payload);
        },
        setCurrentStudies(state, action: PayloadAction<CurrentStudies>){
            state.studies = action.payload.studies
        }
    },
})

// Action creators are generated for each case reducer function
export const { logMessage, setRunning, setRunningDetail, setCurrentStudies } = mainSlice.actions

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
export const selectRunning = (state: RootState) => state.main.running