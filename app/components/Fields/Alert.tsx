import {classNames} from "@/app/lib/helpers";
import React from "react";

export default function Alert({ children, type = 'success'} : { children: React.ReactNode, type?: string}) {

    const classes: {[key: string] : string} = {
       success: 'bg-green-50 text-green-700',
       warning: 'bg-orange-50 text-orange-700',
       error: 'bg-red-50 text-red-700',
    };

    return (
        <div className={classNames("rounded-md  p-4 mb-4 shadow-lg text-sm", classes[type])}>
            {children}
        </div>
    )
}