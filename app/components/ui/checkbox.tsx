import * as React from "react"

import { cn } from "@/app/lib/utils"

export interface CheckboxProps
    extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "type"> {
    indeterminate?: boolean,
}

const Checkbox = React.forwardRef<HTMLInputElement, CheckboxProps>(
    ({ className, indeterminate = false, ...props }, ref) => {
        const innerRef = React.useRef<HTMLInputElement | null>(null)

        React.useImperativeHandle<HTMLInputElement | null, HTMLInputElement | null>(
            ref,
            () => innerRef.current,
        )

        React.useEffect(() => {
            if (innerRef.current) {
                innerRef.current.indeterminate = indeterminate
            }
        }, [indeterminate])

        return (
            <input
                ref={innerRef}
                type="checkbox"
                className={cn(
                    "h-4 w-4 cursor-pointer rounded border border-input bg-background text-primary shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
                    className,
                )}
                {...props}
            />
        )
    },
)
Checkbox.displayName = "Checkbox"

export { Checkbox }
