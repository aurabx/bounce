

import { Input } from "@/app/components/ui/input"
import { Label } from "@/app/components/ui/label"
import { Button } from "@/app/components/ui/button"

export default function TextInput(props: {
    config: { help?: string | null; label: string; key: string, suffix_button?: string | null, readOnly?: boolean },
    settings?: { [p: string]: any } | undefined | null,
    value: any,
    onChange: (e: any) => void
    onSuffixClick?: (e: any) => void
}) {
    return <div className="space-y-2">
        <Label htmlFor={props.config.key}>
            {props.config.label}
        </Label>
        <div className="flex gap-x-2">
            <Input
                id={props.config.key}
                name={props.config.key}
                value={props.value}
                readOnly={!!props.config.readOnly}
                onChange={props.onChange}
                className="bg-white"
            />
            {props.config.suffix_button ? <Button
                variant="outline"
                type="button"
                onClick={props.onSuffixClick}>
                {props.config.suffix_button}
            </Button> : null}
        </div>
        {props.config.help && <p className="text-sm text-muted-foreground">{props.config.help}</p>}
    </div>;
}
