'use client';

import { invoke } from '@tauri-apps/api/core';
import {receiverStart, receiverStop} from "@/app/lib/server";
import {useAppSelector} from "@/app/lib/hook";
import {classNames} from "@/app/lib/helpers";
import {useSetupComplete} from "@/app/lib/customHooks";
import Settings from "@/app/components/Settings";

export default function Page() {
    const running = useAppSelector((state) => state.main.running)
    const runningDetail = useAppSelector((state) => state.main.runningDetail)
    const studies = useAppSelector((state) => state.main.studies)
    const errors = useAppSelector((state) => state.main.errors)
    const {setupComplete} =  useSetupComplete();

    const sendLog = async () => {
        try {
            await invoke('send_log', { log: "Log and things" }); // Pass the port to Tauri
        } catch (error) {
            console.error('Error starting server:', error);
            alert(`Failed to start server: ${error}`);
        }
    };

    const loadStudies = async () => {
        await invoke('current_studies');
    }

    return (setupComplete ?
        <main className="">
            <h1 className="text-3xl mt-6 font-bold mb-12 text-center">
                Aurabox Bounce
            </h1>

            <div className="flex justify-center mb-12">
                <button
                    onClick={running ? receiverStop : receiverStart}
                    className="flex transition ease-in-out text-center border shadow-sm font-medium rounded-md px-8 py-4 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                >
                    {running ? 'Stop Service' : 'Start Service'}
                </button>
            </div>

            {errors.length > 0 && (
                <ul className="bg-red-100 border border-red-400 text-red-700 flex flex-col gap-2 rounded-lg">
                    {errors.map((error, index) => (
                        <li key={`error-${index}`} className="p-2">{error}</li>
                    ))}
                </ul>
            )}

            <div className="mb-6">
                <dl className="mt-5 grid grid-cols-1 divide-y divide-gray-200 overflow-hidden rounded-lg bg-white shadow-sm md:grid-cols-2 md:divide-x md:divide-y-0">
                    <div className="px-4 py-5 sm:p-6">
                        <dt className="text-base font-normal text-gray-900">Status</dt>
                        <dd className="mt-1 flex items-baseline justify-between">
                            <div className="flex items-baseline text-2xl font-semibold text-indigo-600">
                                {running ? 'Running' : 'Stopped'}
                            </div>
                            <div
                                className={classNames(
                                    running
                                        ? 'bg-emerald-400 shadow-lg shadow-emerald-300'
                                        : 'bg-indigo-200 hover:bg-indigo-500 hover:text-white',
                                    'inline-flex items-baseline rounded-full h-4 w-4 text-sm font-medium md:mt-2 lg:mt-0',
                                )}>
                            </div>
                        </dd>
                    </div>
                    <div className="px-4 py-5 sm:p-6">
                        <dt className="text-base font-normal text-gray-900">Current studies</dt>
                        <dd className="mt-1 flex items-baseline justify-between md:block lg:flex">
                            <div className="flex items-baseline text-2xl font-semibold text-indigo-600">
                                {studies.length}
                            </div>
                        </dd>
                    </div>
                </dl>
            </div>

            {running && (<div className="mb-12">
                <div className="overflow-hidden bg-white shadow-sm sm:rounded-lg p-4">
                    <div className="divide-y divide-zinc-100">
                        {runningDetail.map((detail, key) => (
                        <dl key={key}>
                            <div className="py-3 sm:grid sm:grid-cols-3 sm:gap-4">
                                <dt className="text-sm text-zinc-500 font-semibold">{detail.label}</dt>
                                <dd className="mt-1 text-sm text-zinc-900 sm:mt-0 sm:col-span-2 break-words">{detail.value}</dd>
                            </div>
                        </dl>
                        ))}
                    </div>
                </div>
            </div>)}

            <div className="bg-white shadow-lg rounded-lg p-6 w-full hidden">
                <div className="flex gap-4">
                    <button
                        onClick={loadStudies}
                        className="flex w-full  transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                    >
                        Load current studies
                    </button>
                    <button
                        onClick={sendLog}
                        className="flex w-full  transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                    >
                        Send Log
                    </button>
                </div>
            </div>
        </main> : (
                <div className="relative p-4">
                    <h1 className="text-3xl mt-6 font-bold mb-6 text-center">
                        Welcome to Aurabox Bounce
                    </h1>
                    <h3 className="text-xl mt-6 mb-6 text-center text-slate-500">
                        Please add your api key, port and local storage location below
                    </h3>
                    <Settings/>
                </div>
            )
    );
}
