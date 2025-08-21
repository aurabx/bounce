'use client';

import {Suspense, useEffect, useState} from 'react'
import {useAppSelector} from "@/app/lib/hook";
import {invoke} from "@tauri-apps/api/core";
import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react'
import { EllipsisVerticalIcon } from '@heroicons/react/20/solid'
import {classNames, formatDicomDateAndTime} from "@/app/lib/helpers";
import {Study} from "@/app/lib/types";

const statuses = {
    COMPLETE: 'text-green-700 bg-green-50 ring-green-600/20',
    SENT: 'text-green-700 bg-green-50 ring-green-600/20',
    "IN-PROGRESS": 'text-yellow-600 bg-yellow-50 ring-yellow-500/10',
    ARCHIVED: 'text-blue-800 bg-blue-50 ring-blue-600/20',
    UNKNOWN: 'text-gray-800 bg-gray-50 ring-gray-600/20',
}

export default function Page() {

    const [loaded, setLoaded] = useState<boolean>(false);

    const studies = useAppSelector((state) => state.main.studies)

    useEffect(() => {
        invoke('current_studies').then(() => {
            setLoaded(true)
        });
    }, []);

    const sendStudy = async (study: Study) => {
        await invoke('send_study', {
            studyUid: study.study_uid,
        });
    };

    const deleteStudy = async (study: Study) => {
        setLoaded(false)

        await invoke('delete_study', {
            studyUid: study.study_uid,
        });

        invoke('current_studies').then(() => {
            setLoaded(true)
        });
    };

    return (
        <div className="h-full flex flex-col">
            <div>
                <Suspense fallback={<Loading/>}>
                    {(loaded ? <div className="h-full">
                        <ul role="list" className="divide-y divide-gray-100 space-y-2">
                            {studies.map((study) => (
                                <li key={study.study_uid} className="flex items-center justify-between gap-x-6 py-5 bg-white shadow-lg rounded-lg p-6 w-full flex-grow">
                                    <div className="min-w-0">
                                        <div className="flex items-start gap-x-3">
                                            <p className="text-sm/6 font-semibold text-gray-900">{study.study_description}</p>
                                            <p
                                                className={classNames(
                                                    statuses.hasOwnProperty(study.status) ? statuses[study.status as keyof typeof statuses] : statuses.UNKNOWN,
                                                    'mt-0.5 rounded-md px-1.5 py-0.5 text-xs font-medium whitespace-nowrap ring-1 ring-inset',
                                                )}
                                            >
                                                {study.status}
                                            </p>
                                        </div>
                                        <div className="mt-1 flex flex-col items-start gap-x-2 text-xs/5 text-gray-500">
                                            <p className="whitespace-nowrap truncate w-full">
                                                Study Instance UID: {study.study_uid}
                                            </p>
                                            <p className="truncate w-full">Created at: {formatDicomDateAndTime(study.study_date, study.study_time)}</p>
                                        </div>
                                    </div>
                                    <div className="flex flex-none items-center gap-x-4">
                                        <a
                                            href="#"
                                            onClick={() => sendStudy(study)}
                                            className="hidden rounded-md bg-white px-2.5 py-1.5 text-sm font-semibold text-gray-900 ring-1 shadow-xs ring-gray-300 ring-inset hover:bg-gray-50 sm:block"
                                        >
                                            Send study <span className="sr-only">, {study.study_description}</span>
                                        </a>
                                        <Menu as="div" className="relative flex-none">
                                            <MenuButton className="-m-2.5 block p-2.5 text-gray-500 hover:text-gray-900">
                                                <span className="sr-only">Open options</span>
                                                <EllipsisVerticalIcon aria-hidden="true" className="size-5" />
                                            </MenuButton>
                                            <MenuItems
                                                transition
                                                className="absolute right-0 z-10 mt-2 w-32 origin-top-right rounded-md bg-white py-2 ring-1 shadow-lg ring-gray-900/5 transition focus:outline-hidden data-closed:scale-95 data-closed:transform data-closed:opacity-0 data-enter:duration-100 data-enter:ease-out data-leave:duration-75 data-leave:ease-in"
                                            >
                                                <MenuItem>
                                                    <a
                                                        href="#"
                                                        onClick={() => deleteStudy(study)}
                                                        className="block px-3 py-1 text-sm/6 text-gray-900 data-focus:bg-gray-50 data-focus:outline-hidden"
                                                    >
                                                        Delete<span className="sr-only">, {study.study_description}</span>
                                                    </a>
                                                </MenuItem>
                                            </MenuItems>
                                        </Menu>
                                    </div>
                                </li>
                            ))}
                        </ul>
                    </div> : null)}
                </Suspense>
            </div>
        </div>
    );
}


function Loading() {
    return <h2>🌀 Loading...</h2>;
}