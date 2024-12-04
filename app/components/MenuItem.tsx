import {classNames} from "@/app/helpers";


export default function MenuItem(props: { href: string, label: string, current: string  }) {
    return <li>
        <a
            href={props.href}
            className={classNames(
                props.current
                    ? 'bg-indigo-700 text-white'
                    : 'text-indigo-200 hover:bg-indigo-700 hover:text-white',
                'group flex gap-x-3 rounded-md p-2 text-sm/6 font-semibold',
            )}
        >
            {props.label}
        </a>
    </li>;
}