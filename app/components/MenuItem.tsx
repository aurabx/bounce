import {classNames} from "@/app/helpers";


export default function MenuItem({
         href = '',
         label = '',
         current = false
    } : {
        href: string,
        label: string,
        current: boolean
    }) {
    return <li>
        <a
            href={href}
            className={classNames(
                current
                    ? 'bg-indigo-700 text-white'
                    : 'text-indigo-200 hover:bg-indigo-700 hover:text-white',
                'group flex gap-x-3 rounded-md p-2 text-sm/6 font-semibold',
            )}
        >
            {label}
        </a>
    </li>;
}