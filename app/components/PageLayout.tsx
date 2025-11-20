'use client';

import React from "react";
import Sidebar from "@/app/components/Sidebar";
import PageTitle from "@/app/components/PageTitle";
import {useSetupComplete} from "@/app/lib/customHooks";

export default function PageLayout({children}: { children: React.ReactNode }) {
    const {setupComplete, isLoading} = useSetupComplete();

    return <div className="h-full relative bg-muted/20">
        {setupComplete ? (
            <>
                <Sidebar/>
                <div className="pl-64 h-full flex flex-col">
                    <PageTitle/>
                    <main className="flex-1 overflow-y-auto p-8">
                        {children}
                    </main>
                </div>
            </>
        ) : <>{children}</>}

    </div>;
}