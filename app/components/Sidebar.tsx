"use client";

import React, {useEffect, useState} from "react";
import {items} from "@/app/lib/menu";
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

    return <div className="fixed inset-y-0 flex w-52 flex-col bg-background border-r">
        <div className="flex h-16 shrink-0 items-center px-4 font-bold text-base tracking-tight border-b text-primary">
            Aurabox Bounce
        </div>
        <div className="flex grow flex-col gap-y-5 overflow-y-auto px-3 pb-4 pt-4">
            <nav className="flex flex-1 flex-col">
                <ul role="list" className="flex flex-1 flex-col gap-y-2">
                    <li>
                        <ul role="list" className="space-y-1">
                            {items.map(
                                (item) => <MenuItem key={item.label} {...item}/>
                            )}
                        </ul>
                    </li>
                    <li className="mt-auto">
                        <CurrentStatus/>
                        <div className="mt-4 px-2">
                             <span className="text-xs font-medium text-muted-foreground">
                                v{appVersion}
                            </span>
                        </div>
                    </li>
                </ul>
            </nav>
        </div>
    </div>;
}
