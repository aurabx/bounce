"use client"

import { cn } from "@/app/lib/utils"
import { usePathname } from "next/navigation"
import Link from 'next/link'
import { buttonVariants } from "@/app/components/ui/button"

export default function MenuItem({
         href = '',
         label = '',
         icon: Icon,
    } : {
        href: string,
        label: string,
        icon: any,
    }) {
    const pathName = usePathname();

    return <li>
        <Link
            href={href}
            className={cn(
                buttonVariants({ variant: "ghost" }),
                "w-full justify-start gap-x-3",
                pathName === href
                    ? 'bg-primary/10 text-primary hover:bg-primary/20 hover:text-primary'
                    : 'text-muted-foreground hover:text-primary'
            )}
        >
            <Icon className="h-5 w-5 shrink-0" aria-hidden="true" />
            {label}
        </Link>
    </li>;
}
