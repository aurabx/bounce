

import { Label } from "@/app/components/ui/label"
import { cn } from "@/app/lib/utils"

export default function SelectInput(props: {
    config: { options: { [key: string]: string }; label: string; key: string, help?: string },
    settings?: { [p: string]: any } | undefined,
    value: any,
    onChange: (e: any) => void
}) {
    return <div className="space-y-2">
        <Label htmlFor={props.config.key}>
            {props.config.label}
        </Label>
        <div>
            <select
                id={props.config.key}
                name={props.config.key}
                onChange={props.onChange}
                value={props.value}
                className={cn(
                    "flex h-9 w-full items-center justify-between rounded-md border border-input bg-white px-3 py-2 text-sm shadow-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                )}
            >
                {Object.entries(props.config.options).map(([value,label]) => (
                    <option
                        value={value}
                        key={value}
                    >{label}</option>
                ))}
            </select>
            {props.config.help && <p className="text-sm text-muted-foreground mt-1">{props.config.help}</p>}
        </div>
    </div>;
}
