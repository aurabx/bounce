'use client';

import { invoke } from '@tauri-apps/api/core';
import {receiverStart, receiverStop} from "@/app/lib/server";
import {useAppSelector} from "@/app/lib/hook";
import {classNames} from "@/app/lib/helpers";

export default function Page() {
    const running = useAppSelector((state) => state.main.running)
    const studies = useAppSelector((state) => state.main.studies)

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

    return (
        <main className="">
            <h1 className="text-3xl mt-6 font-bold mb-12 text-center">
                Aurabox Bounce
            </h1>

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
            <div className="bg-white shadow-lg rounded-lg p-6 w-full">
                <div className="mb-4">
                    <button
                        onClick={running ? receiverStop : receiverStart}
                        className="flex w-full  transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                    >
                        {running ? 'Stop Service' : 'Start Service'}
                    </button>
                </div>
                <div className="mb-4 flex gap-4">
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
        </main>
    );
}
