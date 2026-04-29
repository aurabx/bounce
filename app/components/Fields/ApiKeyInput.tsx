'use client';

import { useState } from 'react';
import { Input } from "@/app/components/ui/input";
import { Label } from "@/app/components/ui/label";
import { Button } from "@/app/components/ui/button";

const MASKED_DISPLAY = '••••••••••••••••••••';

/**
 * API key input with a one-way reveal model.
 *
 * Once a key has been saved (`settings._has_api_key === true`) the real
 * value is never exposed back to the UI: the field renders a masked
 * placeholder and a "Change API Key" button. To replace the key the
 * operator must enter a new one — there is no way to copy the existing
 * key out of the UI.
 *
 * When entering a key the input uses `type="password"` so it isn't
 * shoulder-readable, and `autoComplete="off"` to keep it out of browser
 * password stores.
 */
export default function ApiKeyInput(props: {
    config: { help?: string | null; label: string; key: string },
    settings?: { [p: string]: any } | undefined | null,
    value: any,
    onChange: (e: any) => void
}) {
    const hasExistingKey = props.settings?._has_api_key === true;
    const [editing, setEditing] = useState(false);
    const showInput = !hasExistingKey || editing;

    const cancelEdit = () => {
        setEditing(false);
        props.onChange({ target: { value: '' } });
    };

    const startEdit = () => {
        setEditing(true);
    };

    return (
        <div className="space-y-2">
            <Label htmlFor={props.config.key}>
                {props.config.label}
            </Label>
            <div className="flex gap-x-2">
                {showInput ? (
                    <Input
                        id={props.config.key}
                        name={props.config.key}
                        type="password"
                        autoComplete="off"
                        spellCheck={false}
                        placeholder={hasExistingKey ? 'Enter a new API key to replace the existing one' : 'aura_…'}
                        value={props.value ?? ''}
                        onChange={props.onChange}
                        className="bg-white font-mono"
                    />
                ) : (
                    <Input
                        id={props.config.key}
                        name={props.config.key}
                        readOnly
                        tabIndex={-1}
                        aria-label="API key is set"
                        value={MASKED_DISPLAY}
                        onFocus={(e) => e.currentTarget.blur()}
                        className="bg-gray-50 font-mono select-none cursor-not-allowed text-muted-foreground"
                    />
                )}
                {hasExistingKey && (
                    editing ? (
                        <Button type="button" variant="outline" onClick={cancelEdit}>
                            Cancel
                        </Button>
                    ) : (
                        <Button type="button" variant="outline" onClick={startEdit}>
                            Change API Key
                        </Button>
                    )
                )}
            </div>
            {hasExistingKey && !editing && (
                <p className="text-sm text-muted-foreground">
                    Your API key is saved and hidden. To replace it, click &quot;Change API Key&quot; — the existing key cannot be revealed.
                </p>
            )}
            {(!hasExistingKey || editing) && props.config.help && (
                <p className="text-sm text-muted-foreground">{props.config.help}</p>
            )}
        </div>
    );
}
