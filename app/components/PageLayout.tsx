'use client';

import React from "react";
import Sidebar from "@/app/components/Sidebar";
import PageTitle from "@/app/components/PageTitle";

export default function PageLayout(props: { children: React.ReactNode }) {
    return <div className="h-full relative">
        <Sidebar/>
        <div className="pl-48 h-full">
            <main className="pb-4 h-full">
                <PageTitle/>
                <div className="p-4">{props.children}</div>
            </main>
        </div>
    </div>;
}