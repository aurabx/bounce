'use client';

import { useEffect } from 'react';
import { CheckCircleIcon, XCircleIcon, ArrowPathIcon, MinusCircleIcon } from '@heroicons/react/24/outline';
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui/card";
import { Button } from "@/app/components/ui/button";
import { cn } from "@/app/lib/utils";
import { useAppDispatch, useAppSelector } from "@/app/lib/hook";
import { verifyConnectivity } from "@/app/lib/server";

const STATUS_LABEL: Record<string, string> = {
    idle: 'Not checked',
    checking: 'Checking…',
    ok: 'Connected',
    failed: 'Unreachable',
};

function formatTimestamp(ts: number | null): string | null {
    if (!ts) return null;
    const d = new Date(ts);
    return d.toLocaleTimeString();
}

/**
 * Dashboard card showing the current Aurabox connectivity state. Reads from
 * the shared Redux slice so it reflects checks initiated by Settings (on
 * mount and on save) as well as manual rechecks from this card.
 */
export default function ConnectivityStatus() {
    const dispatch = useAppDispatch();
    const { status, error, lastCheckedAt } = useAppSelector((state) => state.main.connectivity);

    // Trigger an initial check the first time this card is rendered if no
    // check has happened yet this session.
    useEffect(() => {
        if (status === 'idle') {
            verifyConnectivity(dispatch).then();
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    const onRecheck = () => {
        if (status === 'checking') return;
        verifyConnectivity(dispatch).then();
    };

    const Icon = status === 'ok'
        ? CheckCircleIcon
        : status === 'failed'
            ? XCircleIcon
            : status === 'checking'
                ? ArrowPathIcon
                : MinusCircleIcon;

    const iconColor = status === 'ok'
        ? 'text-emerald-500'
        : status === 'failed'
            ? 'text-red-500'
            : status === 'checking'
                ? 'text-muted-foreground'
                : 'text-muted-foreground';

    const checkedAt = formatTimestamp(lastCheckedAt);

    return (
        <Card>
            <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground">Aurabox connectivity</CardTitle>
            </CardHeader>
            <CardContent>
                <div className="flex items-center justify-between gap-4">
                    <div className="flex items-center gap-3 min-w-0">
                        <Icon className={cn(
                            'h-8 w-8 shrink-0',
                            iconColor,
                            status === 'checking' && 'animate-spin',
                        )} />
                        <div className="min-w-0">
                            <div className="text-2xl font-bold text-primary">
                                {STATUS_LABEL[status]}
                            </div>
                            {status === 'failed' && error && (
                                <div className="text-xs text-red-600 truncate" title={error}>
                                    {error}
                                </div>
                            )}
                            {status !== 'failed' && checkedAt && (
                                <div className="text-xs text-muted-foreground">
                                    Last checked {checkedAt}
                                </div>
                            )}
                        </div>
                    </div>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={onRecheck}
                        disabled={status === 'checking'}
                    >
                        Recheck
                    </Button>
                </div>
            </CardContent>
        </Card>
    );
}
