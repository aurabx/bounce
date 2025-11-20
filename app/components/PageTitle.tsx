'use client';

import {usePathname} from "next/navigation";
import React from "react";
import {resolveTitleFromPath} from "@/app/lib/menu";



export default function PageTitle() {
    const pathname = usePathname()
    const title = resolveTitleFromPath(pathname)

    if (!title){
        return null;
    }

    return <div className="sticky top-0 z-10 flex h-16 items-center border-b bg-background px-8 text-xl font-semibold">
        {title}
    </div>
}

