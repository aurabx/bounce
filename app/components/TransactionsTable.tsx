'use client'

import {useState} from "react";
import {UploadAttempt} from "@/app/lib/types";
import {Badge} from "@/app/components/ui/badge";
import {cn} from "@/app/lib/utils";

// Maps the backend upload-attempt status to a badge variant and label.
// Statuses are written by the upload pipeline (src-tauri/src/db/database.rs):
// STARTED when an attempt is claimed, SUCCESS/FAILED when it resolves.
const statusMap: Record<string, {
    variant: "default" | "secondary" | "destructive" | "outline" | "success" | "warning",
    label: string,
}> = {
    STARTED: {variant: 'warning', label: 'Started'},
    SUCCESS: {variant: 'success', label: 'Success'},
    FAILED: {variant: 'destructive', label: 'Failed'},
}

export interface TransactionsTableProps {
    attempts: UploadAttempt[],
}

// A clickable cell that copies an identifier to the clipboard. Long values
// (Study UID, upload id) are shown truncated with the full value in `title`;
// after a successful copy the label briefly reads "Copied" so the action is
// acknowledged inline without a toast component. Mirrors the copy affordance
// used in StudiesTable but kept local to avoid coupling the two tables.
function CopyableText({value, display}: { value: string, display: string }) {
    const [copied, setCopied] = useState<boolean>(false);

    const handleCopy = async (e: React.MouseEvent) => {
        e.stopPropagation();
        try {
            await navigator.clipboard.writeText(value);
            setCopied(true);
            window.setTimeout(() => setCopied(false), 1500);
        } catch (err) {
            console.error('Failed to copy to clipboard:', err);
        }
    };

    return (
        <button
            type="button"
            onClick={handleCopy}
            title={copied ? 'Copied to clipboard' : `${value}\n\nClick to copy`}
            className={cn(
                "inline-flex max-w-full items-center truncate rounded-md border border-transparent px-2 py-1 font-mono text-xs leading-tight",
                "hover:border-input hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                copied && "border-input bg-emerald-50 text-emerald-700",
            )}
            aria-label={copied ? 'Copied' : `Copy ${value}`}
        >
            {copied ? 'Copied' : display}
        </button>
    );
}

// Last three dot-separated segments of a DICOM UID, enough to distinguish
// rows while the full value remains available via copy/hover.
function shortenUid(uid: string): string {
    const tail = uid.split('.').slice(-3).join('.');
    return tail.length > 0 ? `…${tail}` : uid;
}

// Format an ISO timestamp as a coarse relative interval, with the absolute
// time exposed for hover. Returns "—" for missing/unparseable input. Kept
// local so the table pulls in no date library.
function formatRelative(iso?: string | null): { rel: string, abs: string } {
    if (!iso) return {rel: '—', abs: ''}

    const date = new Date(iso)
    const ms = date.getTime()
    if (Number.isNaN(ms)) return {rel: '—', abs: iso}

    const diffSec = Math.round((Date.now() - ms) / 1000)
    const abs = date.toLocaleString()

    if (diffSec < 60) return {rel: `${diffSec}s ago`, abs}
    if (diffSec < 3600) return {rel: `${Math.floor(diffSec / 60)}m ago`, abs}
    if (diffSec < 86400) return {rel: `${Math.floor(diffSec / 3600)}h ago`, abs}
    if (diffSec < 86400 * 7) return {rel: `${Math.floor(diffSec / 86400)}d ago`, abs}

    return {rel: date.toLocaleDateString(), abs}
}

// Human-readable duration from milliseconds: "—" when absent, "850ms" under
// a second, otherwise seconds with one decimal.
function formatDuration(ms?: number | null): string {
    if (ms === null || ms === undefined) return '—'
    if (ms < 1000) return `${ms}ms`
    return `${(ms / 1000).toFixed(1)}s`
}

export default function TransactionsTable({attempts}: TransactionsTableProps) {
    return (
        <div className="rounded-md border overflow-x-auto">
            <table className="w-full table-fixed text-sm">
                <colgroup>
                    <col style={{width: '13rem'}}/>
                    <col style={{width: '5rem'}}/>
                    <col style={{width: '7rem'}}/>
                    <col style={{width: '8rem'}}/>
                    <col style={{width: '6rem'}}/>
                    <col style={{width: '12rem'}}/>
                    <col/>
                </colgroup>
                <thead className="bg-muted/40 text-muted-foreground">
                    <tr>
                        <th className="px-3 py-2 text-left font-medium">Study UID</th>
                        <th className="px-3 py-2 text-left font-medium">Attempt</th>
                        <th className="px-3 py-2 text-left font-medium">Status</th>
                        <th className="px-3 py-2 text-left font-medium">Started</th>
                        <th className="px-3 py-2 text-left font-medium">Duration</th>
                        <th className="px-3 py-2 text-left font-medium">Upload ID</th>
                        <th className="px-3 py-2 text-left font-medium">Error</th>
                    </tr>
                </thead>
                <tbody>
                    {attempts.map((attempt) => {
                        const status = statusMap[attempt.status]
                        const {rel, abs} = formatRelative(attempt.started_at)
                        const uploadId = attempt.upload_id?.trim()
                        const error = attempt.error?.trim()

                        return (
                            <tr
                                key={attempt.id ?? `${attempt.study_uid}-${attempt.attempt_no}-${attempt.started_at}`}
                                className="border-t transition-colors hover:bg-muted/40"
                            >
                                <td className="px-3 py-2 align-top overflow-hidden">
                                    <CopyableText
                                        value={attempt.study_uid}
                                        display={shortenUid(attempt.study_uid)}
                                    />
                                </td>
                                <td className="px-3 py-2 align-top tabular-nums">
                                    {attempt.attempt_no}
                                </td>
                                <td className="px-3 py-2 align-top">
                                    <Badge variant={status?.variant ?? 'outline'}>
                                        {status?.label ?? attempt.status}
                                    </Badge>
                                </td>
                                <td
                                    className="px-3 py-2 align-top whitespace-nowrap overflow-hidden text-ellipsis"
                                    title={abs}
                                >
                                    {rel}
                                </td>
                                <td className="px-3 py-2 align-top tabular-nums whitespace-nowrap">
                                    {formatDuration(attempt.duration_ms)}
                                </td>
                                <td className="px-3 py-2 align-top overflow-hidden">
                                    {uploadId
                                        ? <CopyableText value={uploadId} display={uploadId}/>
                                        : <span className="text-muted-foreground">—</span>}
                                </td>
                                <td className="px-3 py-2 align-top">
                                    {error
                                        ? <span
                                            className="block truncate text-destructive"
                                            title={error}
                                        >
                                            {error}
                                        </span>
                                        : <span className="text-muted-foreground">—</span>}
                                </td>
                            </tr>
                        )
                    })}
                </tbody>
            </table>
        </div>
    )
}
