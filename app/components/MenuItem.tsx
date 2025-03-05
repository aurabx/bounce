"use client"

import {classNames} from "@/app/lib/helpers";
import {usePathname} from "next/navigation";
import Link from 'next/link'

export default function MenuItem({
         href = '',
         label = '',
    } : {
        href: string,
        label: string,
    }) {
    const pathName = usePathname();

    return <li>
        <Link
            href={href}
            className={classNames(
                pathName === href
                    ? 'bg-indigo-700 text-white'
                    : 'text-indigo-200 hover:bg-indigo-700 hover:text-white',
                'group flex gap-x-3 rounded-md p-2 text-sm/6 font-semibold',
            )}
        >
            {label}
        </Link>
    </li>;
}