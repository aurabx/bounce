'use client';

import {useCallback, useEffect, useState} from 'react';
import Link from 'next/link';
import {invokeCommand} from '@/app/lib/commands';
import {receiverStart, receiverStop} from "@/app/lib/server";
import {useAppSelector} from "@/app/lib/hook";
import {cn} from "@/app/lib/utils";
import {useSetupComplete} from "@/app/lib/customHooks";
import {DashboardStats, UploadAttempt} from "@/app/lib/types";
import Settings from "@/app/components/Settings";
import ConnectivityStatus from "@/app/components/ConnectivityStatus";
import RecentTransactions from "@/app/components/RecentTransactions";
import {Button} from "@/app/components/ui/button";
import {Card, CardContent, CardHeader, CardTitle} from "@/app/components/ui/card";
import {Alert, AlertDescription} from "@/app/components/ui/alert";

// How often the dashboard re-polls aggregate stats and recent transactions.
// Studies arrive in the background via DICOM, so a short interval keeps the
// summary live without the cost of streaming every change into the UI.
const REFRESH_INTERVAL_MS = 5000;
const RECENT_TRANSACTIONS_LIMIT = 5;

// Coarse relative-time formatter for the "last received" subtext. Kept local
// and minimal so the dashboard pulls in no date library; mirrors the coarser
// formatting used in the studies and transactions tables.
function formatRelative(iso: string | null): string {
    if (!iso) return 'never';

    const ms = new Date(iso).getTime();
    if (Number.isNaN(ms)) return 'unknown';

    const diffSec = Math.round((Date.now() - ms) / 1000);
    if (diffSec < 60) return `${Math.max(diffSec, 0)}s ago`;
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
    return `${Math.floor(diffSec / 86400)}d ago`;
}

// A single summary tile. When `href` is set the whole card becomes a link to
// the relevant view, giving the operator a one-click path from a count to the
// rows behind it.
function StatCard({title, href, children}: {
    title: string,
    href?: string,
    children: React.ReactNode,
}) {
    const card = (
        <Card className={cn(href && "transition-colors hover:border-primary/40 hover:bg-muted/30")}>
            <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground">{title}</CardTitle>
            </CardHeader>
            <CardContent>{children}</CardContent>
        </Card>
    );

    return href ? (
        <Link href={href} className="block focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-lg">
            {card}
        </Link>
    ) : card;
}

export default function Page() {
    const running = useAppSelector((state) => state.main.running)
    const runningDetail = useAppSelector((state) => state.main.runningDetail)
    const errors = useAppSelector((state) => state.main.errors)
    const {setupComplete} = useSetupComplete();

    const [stats, setStats] = useState<DashboardStats | null>(null);
    const [recentAttempts, setRecentAttempts] = useState<UploadAttempt[]>([]);
    const [loadingSummary, setLoadingSummary] = useState<boolean>(true);

    // Fetch the aggregate counts and the most recent attempts together so the
    // summary cards and the transactions list always reflect the same moment.
    // Errors are logged rather than surfaced as a banner: a transient failed
    // poll should not replace the last-known-good numbers with an alert.
    const refreshSummary = useCallback(async () => {
        try {
            const [nextStats, attempts] = await Promise.all([
                invokeCommand('dashboard_stats'),
                invokeCommand('current_upload_attempts', {page: 1, limit: RECENT_TRANSACTIONS_LIMIT}),
            ]);
            setStats(nextStats);
            setRecentAttempts(attempts?.attempts ?? []);
        } catch (error) {
            console.error('Failed to refresh dashboard summary:', error);
        } finally {
            setLoadingSummary(false);
        }
    }, []);

    // Poll while the dashboard is mounted and setup is complete. The `running`
    // dependency forces an immediate refresh when the service is started or
    // stopped so the counts do not lag a toggle by up to one interval.
    useEffect(() => {
        if (!setupComplete) return;

        refreshSummary();
        const handle = window.setInterval(refreshSummary, REFRESH_INTERVAL_MS);
        return () => window.clearInterval(handle);
    }, [setupComplete, running, refreshSummary]);

    const summaryValue = (value: number | undefined): string =>
        stats ? String(value ?? 0) : '—';

    return (setupComplete ?
        <main className="space-y-8">
            <div className="flex flex-col items-center justify-center space-y-6 py-8">
                <h1 className="text-4xl font-bold tracking-tight">
                    Aurabox Bounce
                </h1>

                <Button
                    size="lg"
                    onClick={running ? receiverStop : receiverStart}
                    variant={running ? "destructive" : "default"}
                    className="h-16 px-8 text-lg"
                >
                    {running ? 'Stop Service' : 'Start Service'}
                </Button>
            </div>

            {errors.length > 0 && (
                <Alert variant="destructive">
                    <AlertDescription>
                        <ul className="list-disc pl-4 space-y-1">
                            {errors.map((error) => (
                                <li key={error.id}>{error.message}</li>
                            ))}
                        </ul>
                    </AlertDescription>
                </Alert>
            )}

            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
                <StatCard title="Status">
                    <div className="flex items-center justify-between">
                        <div className="text-2xl font-bold text-primary">
                            {running ? 'Running' : 'Stopped'}
                        </div>
                        <div className="flex h-4 w-4 relative">
                            {running && <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>}
                            <span className={cn(
                                running ? 'bg-emerald-500' : 'bg-red-500',
                                "relative inline-flex rounded-full h-4 w-4"
                            )}></span>
                        </div>
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">
                        {stats && stats.total > 0
                            ? `Last received ${formatRelative(stats.last_received_at)}`
                            : 'No studies received yet'}
                    </p>
                </StatCard>

                <StatCard title="Pending uploads" href="/studies">
                    <div className={cn(
                        "text-2xl font-bold",
                        stats && stats.pending > 0 ? "text-amber-600" : "text-primary",
                    )}>
                        {summaryValue(stats?.pending)}
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">
                        {stats
                            ? `${stats.queued} queued · ${stats.retrying} retrying`
                            : 'Queued and retrying studies'}
                    </p>
                </StatCard>

                <StatCard title="Sent" href="/studies">
                    <div className={cn(
                        "text-2xl font-bold",
                        stats && stats.sent > 0 ? "text-emerald-600" : "text-primary",
                    )}>
                        {summaryValue(stats?.sent)}
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">
                        Uploaded to Aurabox
                    </p>
                </StatCard>

                <StatCard title="Failed" href="/studies">
                    <div className={cn(
                        "text-2xl font-bold",
                        stats && stats.failed > 0 ? "text-destructive" : "text-primary",
                    )}>
                        {summaryValue(stats?.failed)}
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">
                        {stats && stats.failed > 0 ? 'Need attention' : 'No failed uploads'}
                    </p>
                </StatCard>
            </div>

            <ConnectivityStatus/>

            <RecentTransactions attempts={recentAttempts} loading={loadingSummary}/>

            {running && (
                <Card>
                    <CardHeader>
                         <CardTitle>Server Details</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                            {runningDetail.map((detail) => (
                                <div key={detail.label} className="space-y-1">
                                    <p className="text-sm font-medium text-muted-foreground">{detail.label}</p>
                                    <p className="text-sm font-mono bg-muted p-2 rounded break-all">{detail.value}</p>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}
        </main> : (
                <div className="max-w-2xl mx-auto py-12 space-y-8">
                    <div className="text-center space-y-2">
                        <h1 className="text-3xl font-bold tracking-tight">
                            Welcome to Aurabox Bounce
                        </h1>
                        <p className="text-muted-foreground">
                            Please configure your API key, port and local storage location below.
                        </p>
                    </div>
                    <Settings/>
                </div>
            )
    );
}
