"use client"

import {classNames} from "@/app/helpers";
import {usePathname} from "next/navigation";


export default function MenuItem({
         href = '',
         label = '',
    } : {
        href: string,
        label: string,
    }) {
    const pathName = usePathname();

    return <li>
        <a
            href={href}
            className={classNames(
                pathName === href
                    ? 'bg-indigo-700 text-white'
                    : 'text-indigo-200 hover:bg-indigo-700 hover:text-white',
                'group flex gap-x-3 rounded-md p-2 text-sm/6 font-semibold',
            )}
        >
            {label}
        </a>
    </li>;
}