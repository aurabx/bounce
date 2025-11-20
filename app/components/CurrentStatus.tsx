"use client"

import { cn } from "@/app/lib/utils";
import { useAppSelector } from "@/app/lib/hook";
import { Button } from "@/app/components/ui/button";

export default function CurrentStatus() {
    const running = useAppSelector((state) => state.main.running)

    return <div className="mt-auto">
        <Button
            variant="outline"
            className="w-full justify-start gap-3 font-normal"
        >
             <div className="relative flex h-2 w-2">
                {running && <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>}
                <span className={cn(
                    running ? 'bg-emerald-500' : 'bg-red-500',
                    "relative inline-flex rounded-full h-2 w-2"
                )}></span>
            </div>
            <span className="text-muted-foreground">{running ? "Running" : "Stopped"}</span>
        </Button>
    </div>;
}
