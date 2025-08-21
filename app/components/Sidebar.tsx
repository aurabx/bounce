"use client";

import React, {useEffect, useState} from "react";
import {items, NavigationItem} from "@/app/lib/menu";
import MenuItem from "@/app/components/MenuItem";
import CurrentStatus from "@/app/components/CurrentStatus";
import { getVersion } from '@tauri-apps/api/app';

export default function Sidebar() {

    const [appVersion, setAppVersion] = useState<string|null>(null);

    useEffect(() => {
        async function fetchVersion() {
            const version = await getVersion();
            setAppVersion(version);
        }
        fetchVersion().then();
    }, []);

    return <div className="fixed inset-y-0 flex w-48 flex-col bg-white">
        <div className="flex grow flex-col overflow-y-auto bg-indigo-600 px-6 py-4">
            <nav className="flex flex-1 flex-col">
                <ul role="list" className="flex flex-col">
                    <li>
                        <ul role="list" className="space-y-2">
                            {items.map(
                                (item) => <MenuItem key={item.label} {...item}/>
                            )}
                        </ul>
                    </li>
                </ul>
                <CurrentStatus/>
                <div>
                    <span className="rounded-full px-3 py-2 text-xs text-indigo-200 font-semibold bg-indigo-500">
                        v{appVersion}
                    </span>
                </div>
            </nav>
        </div>
    </div>;
}