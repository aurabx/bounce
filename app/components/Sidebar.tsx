"use client";

import React, {useEffect, useState} from "react";
import {items, NavigationItem} from "@/app/lib/menu";
// import {useSetupComplete} from "@/app/lib/customHooks";
import MenuItem from "@/app/components/MenuItem";
import CurrentStatus from "@/app/components/CurrentStatus";
import {useSetupComplete} from "@/app/lib/customHooks";

export default function Sidebar() {
    // const [filteredItems, setFilteredItems] = useState<NavigationItem[]>([]);
    const {setupComplete, isLoading} = useSetupComplete();

    return setupComplete ? <div className="fixed inset-y-0 flex w-48 flex-col bg-white">
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
            </nav>
        </div>
    </div> : null;
}