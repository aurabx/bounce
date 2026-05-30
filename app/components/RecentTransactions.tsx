'use client'

import Link from 'next/link'
import {UploadAttempt} from "@/app/lib/types";
import {Card, CardContent, CardHeader, CardTitle} from "@/app/components/ui/card";
import TransactionsTable from "@/app/components/TransactionsTable";

export interface RecentTransactionsProps {
    attempts: UploadAttempt[],
    loading: boolean,
}

/**
 * Dashboard card showing the most recent upload attempts. Purely
 * presentational: the parent owns fetching and passes a pre-sliced list so the
 * dashboard keeps a single polling loop. Reuses {@link TransactionsTable} so
 * the row layout stays consistent with the full Transactions view, and links
 * through to that view for the complete audit trail.
 */
export default function RecentTransactions({attempts, loading}: RecentTransactionsProps) {
    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
                <CardTitle className="text-sm font-medium text-muted-foreground">
                    Recent transactions
                </CardTitle>
                <Link
                    href="/transactions"
                    className="text-sm font-medium text-primary hover:underline"
                >
                    View all
                </Link>
            </CardHeader>
            <CardContent>
                {attempts.length > 0 ? (
                    <TransactionsTable attempts={attempts}/>
                ) : (
                    <p className="py-6 text-center text-sm text-muted-foreground">
                        {loading
                            ? 'Loading…'
                            : 'No upload attempts yet. They will appear here once studies are sent to Aurabox.'}
                    </p>
                )}
            </CardContent>
        </Card>
    );
}
