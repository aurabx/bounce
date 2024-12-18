"use client"

import {classNames} from "@/app/helpers";
import {useAppSelector} from "@/app/lib/hook";
export default function CurrentStatus() {
    const running = useAppSelector((state) => state.main.running)

    return <div className="py-4">
        <button
            className={classNames(
                'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-semibold bg-indigo-500 w-full shadow-inner',
            )}
        >
            <div className="relative group cursor-pointer">
                <div
                    className="absolute -inset-1 bg-gradient-to-r from-emerald-600 to-blue-600 rounded-lg blur opacity-25 group-hover:opacity-100 transition duration-1000">
                </div>
                <div className={classNames(running
                        ? 'bg-emerald-400 shadow-lg shadow-emerald-300'
                        : 'bg-indigo-200 hover:bg-indigo-500 hover:text-white',
                    'relative h-3 w-3  ring-1 ring-gray-900/5 rounded-full leading-none flex items-top')}></div>
            </div>

            <span className="text-indigo-200">{running ? "Running" : "Stopped"}</span>
        </button>
    </div>;
}