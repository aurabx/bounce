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

    return <div className="p-5 bg-white shadow-lg font-semibold text-xl sticky top-0">{title}</div>
}

