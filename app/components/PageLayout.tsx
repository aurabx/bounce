'use client';

import React from "react";
import Sidebar from "@/app/components/Sidebar";
import PageTitle from "@/app/components/PageTitle";
import {useSetupComplete} from "@/app/lib/customHooks";

export default function PageLayout({children}: { children: React.ReactNode }) {
    const {setupComplete, isLoading} = useSetupComplete();

    return <div className="h-full relative">
        {setupComplete ? (
            <>
                <Sidebar/>
                <div className="pl-48 h-full">
                    <main className="pb-4 h-full">
                        <PageTitle/>
                        <div className="p-4">{children}</div>
                    </main>
                </div>
            </>
        ) : <>{children}</>}

    </div>;
}