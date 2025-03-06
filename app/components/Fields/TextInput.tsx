

export default function TextInput(props: {
    config: { help?: string | null; label: string; key: string, suffix_button?: string | null, readOnly?: boolean },
    settings?: { [p: string]: any } | undefined | null,
    value: any,
    onChange: (e: any) => void
    onSuffixClick?: (e: any) => void
}) {
    return <div className="sm:col-span-3">
        <label htmlFor={props.config.key}
               className="block text-sm/6 font-medium text-gray-900">
            {props.config.label}
        </label>
        <div className="mt-2">
            <div className="flex gap-x-2 flex-grow">
                <input
                    id={props.config.key}
                    name={props.config.key}
                    value={props.value}
                    readOnly={!!props.config.readOnly}
                    onChange={props.onChange}
                    className="block w-full rounded-md bg-white px-3 py-1.5 text-base text-gray-900 outline-1 -outline-offset-1 outline-gray-300 placeholder:text-gray-400 focus:outline-2 focus:-outline-offset-2 focus:outline-indigo-600 sm:text-sm/6"
                />
                {props.config.suffix_button ? <button
                    onClick={props.onSuffixClick}
                    className="flex items-center gap-x-2 px-2 py-1 text-sm/6 text-indigo-500 bg-white border-indigo-500 border rounded-md hover:bg-indigo-100 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500">
                    {props.config.suffix_button}
                </button> : null}
            </div>
            <p className="mt-1 text-sm/6 text-gray-600">{props.config.help}</p>
        </div>
    </div>;
}