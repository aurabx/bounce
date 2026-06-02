'use client'

import {useCallback, useEffect, useRef, useState} from "react";
import {Study} from "@/app/lib/types";
import {Badge} from "@/app/components/ui/badge";
import {Button} from "@/app/components/ui/button";
import {Checkbox} from "@/app/components/ui/checkbox";
import {cn} from "@/app/lib/utils";

const statusMap: Record<string, {
    variant: "default" | "secondary" | "destructive" | "outline" | "success" | "warning",
    label: string,
}> = {
    COMPLETE: {variant: 'success', label: 'Complete'},
    SENT: {variant: 'success', label: 'Sent'},
    "IN-PROGRESS": {variant: 'warning', label: 'In Progress'},
    QUEUED: {variant: 'outline', label: 'Queued'},
    UPLOADING: {variant: 'warning', label: 'Uploading'},
    RETRYING: {variant: 'warning', label: 'Retrying'},
    FAILED: {variant: 'destructive', label: 'Failed'},
    ARCHIVED: {variant: 'secondary', label: 'Archived'},
    UNKNOWN: {variant: 'outline', label: 'Unknown'},
}

export interface StudiesTableProps {
    studies: Study[],
    selectedUids: Set<string>,
    onToggleSelect: (uid: string) => void,
    onToggleSelectAll: () => void,
    allOnPageSelected: boolean,
    someOnPageSelected: boolean,
    onSend: (study: Study) => void,
    onRetry: (study: Study) => void,
    onStopRetry: (study: Study) => void,
    onDelete: (study: Study) => void,
}

// Resizable column identifiers. The checkbox column on the far left
// and the actions column on the far right are intentionally fixed —
// resizing them gives little value and complicates layout.
type ResizableKey = 'description' | 'patient' | 'status' | 'received' | 'uid';

const DEFAULT_WIDTHS: Record<ResizableKey, number> = {
    description: 260,
    patient: 240,
    status: 120,
    received: 130,
    uid: 200,
};

const MIN_COL_WIDTH = 80;
const STORAGE_KEY = 'studies-table-col-widths-v1';

// A clickable UID cell that copies the full Study UID to the
// clipboard. The visible label shows the last segment of the UID so
// the operator can distinguish rows; the full UID is in `title` and
// is what gets copied. After a successful copy we briefly swap the
// label for "Copied" so the action is acknowledged inline without a
// toast component.
function CopyableUid({uid}: { uid: string }) {
    const [copied, setCopied] = useState<boolean>(false);

    const segments = uid.split('.');
    const tail = segments.slice(-3).join('.');
    const label = tail.length > 0 ? `…${tail}` : uid;

    const handleCopy = async (e: React.MouseEvent) => {
        e.stopPropagation();
        try {
            await navigator.clipboard.writeText(uid);
            setCopied(true);
            window.setTimeout(() => setCopied(false), 1500);
        } catch (err) {
            console.error('Failed to copy UID:', err);
        }
    };

    return (
        <button
            type="button"
            onClick={handleCopy}
            title={copied ? 'Copied to clipboard' : `${uid}\n\nClick to copy`}
            className={cn(
                "inline-flex max-w-full items-center truncate rounded-md border border-transparent px-2 py-1 font-mono text-xs leading-tight",
                "hover:border-input hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                copied && "border-input bg-emerald-50 text-emerald-700",
            )}
            aria-label={copied ? 'Study UID copied' : `Copy Study UID ${uid}`}
        >
            {copied ? 'Copied' : label}
        </button>
    );
}

// Format an ISO timestamp as a coarse relative interval ("3 minutes ago",
// "5 hours ago", "yesterday"). Returns "—" for unparseable input. Kept
// local so the table does not pull in a date library; the absolute time
// is exposed via the `title` attribute for hover.
function formatRelative(iso?: string): { rel: string, abs: string } {
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

// Hover text for the status badge surfacing retry diagnostics: how many
// attempts have been made, the last error, and when the next retry is due.
function statusTitle(study: Study): string | undefined {
    const parts: string[] = []
    if (typeof study.attempts === 'number' && study.attempts > 0) {
        parts.push(`Attempts: ${study.attempts}`)
    }
    if (study.status === 'RETRYING' && study.next_retry_at) {
        const next = new Date(study.next_retry_at)
        if (!Number.isNaN(next.getTime())) {
            parts.push(`Next retry: ${next.toLocaleString()}`)
        }
    }
    if (study.last_error) {
        parts.push(`Last error: ${study.last_error}`)
    }
    return parts.length > 0 ? parts.join('\n') : undefined
}

/**
 * A draggable handle rendered on the right edge of a resizable
 * column header. Drag tracking is wired to document-level listeners
 * so the cursor can leave the small handle bounds without losing the
 * drag — a 4 px hit-target alone would be far too easy to fall off.
 */
function ResizeHandle({onStart}: { onStart: (e: React.MouseEvent) => void }) {
    return (
        <span
            role="separator"
            aria-orientation="vertical"
            onMouseDown={onStart}
            onDoubleClick={(e) => e.stopPropagation()}
            className={cn(
                "absolute right-0 top-0 h-full w-1.5 cursor-col-resize select-none",
                "hover:bg-primary/30 active:bg-primary/60",
            )}
        />
    );
}

export default function StudiesTable(props: StudiesTableProps) {
    const {
        studies,
        selectedUids,
        onToggleSelect,
        onToggleSelectAll,
        allOnPageSelected,
        someOnPageSelected,
        onSend,
        onRetry,
        onStopRetry,
        onDelete,
    } = props

    // Column widths, hydrated from localStorage on mount. We do not
    // read storage during initial render so the server-rendered HTML
    // matches the first client render (the studies page is a Next.js
    // static export — any divergence triggers a hydration warning).
    const [widths, setWidths] = useState<Record<ResizableKey, number>>(DEFAULT_WIDTHS);

    useEffect(() => {
        try {
            const raw = window.localStorage.getItem(STORAGE_KEY);
            if (!raw) return;
            const parsed = JSON.parse(raw) as Partial<Record<ResizableKey, number>>;
            setWidths((prev) => {
                const next = {...prev};
                (Object.keys(DEFAULT_WIDTHS) as ResizableKey[]).forEach((k) => {
                    const v = parsed[k];
                    if (typeof v === 'number' && v >= MIN_COL_WIDTH) {
                        next[k] = v;
                    }
                });
                return next;
            });
        } catch {
            // Ignore storage failures — fall back to defaults.
        }
    }, []);

    useEffect(() => {
        try {
            window.localStorage.setItem(STORAGE_KEY, JSON.stringify(widths));
        } catch {
            // Ignore quota errors etc.
        }
    }, [widths]);

    // Drag-in-progress state is held in a ref so the document-level
    // mousemove handler does not need to be re-registered on every
    // width update.
    const dragRef = useRef<{key: ResizableKey, startX: number, startW: number} | null>(null);

    const onMouseMove = useCallback((e: MouseEvent) => {
        const drag = dragRef.current;
        if (!drag) return;
        const delta = e.clientX - drag.startX;
        const next = Math.max(MIN_COL_WIDTH, drag.startW + delta);
        setWidths((prev) => prev[drag.key] === next ? prev : {...prev, [drag.key]: next});
    }, []);

    const endResize = useCallback(() => {
        dragRef.current = null;
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', endResize);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
    }, [onMouseMove]);

    const startResize = (key: ResizableKey) => (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        dragRef.current = {key, startX: e.clientX, startW: widths[key]};
        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', endResize);
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    };

    // Defensive cleanup: if the component unmounts mid-drag, drop the
    // document listeners so we do not leak handlers.
    useEffect(() => {
        return () => {
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', endResize);
        };
    }, [onMouseMove, endResize]);

    return (
        <div className="rounded-md border overflow-x-auto">
            <table className="w-full table-fixed text-sm">
                <colgroup>
                    <col style={{width: '2.5rem'}}/>
                    <col style={{width: widths.description}}/>
                    <col style={{width: widths.patient}}/>
                    <col style={{width: widths.status}}/>
                    <col style={{width: widths.received}}/>
                    <col style={{width: widths.uid}}/>
                    <col style={{width: '8.5rem'}}/>
                </colgroup>
                <thead className="bg-muted/40 text-muted-foreground">
                    <tr>
                        <th className="px-3 py-2 text-left font-medium">
                            <Checkbox
                                checked={allOnPageSelected}
                                indeterminate={!allOnPageSelected && someOnPageSelected}
                                onChange={onToggleSelectAll}
                                aria-label="Select all studies on this page"
                            />
                        </th>
                        <th className="relative px-3 py-2 text-left font-medium">
                            <span className="truncate">Description</span>
                            <ResizeHandle onStart={startResize('description')}/>
                        </th>
                        <th className="relative px-3 py-2 text-left font-medium">
                            <span className="truncate">Patient</span>
                            <ResizeHandle onStart={startResize('patient')}/>
                        </th>
                        <th className="relative px-3 py-2 text-left font-medium">
                            <span className="truncate">Status</span>
                            <ResizeHandle onStart={startResize('status')}/>
                        </th>
                        <th className="relative px-3 py-2 text-left font-medium">
                            <span className="truncate">Received</span>
                            <ResizeHandle onStart={startResize('received')}/>
                        </th>
                        <th className="relative px-3 py-2 text-left font-medium">
                            <span className="truncate">UID</span>
                            <ResizeHandle onStart={startResize('uid')}/>
                        </th>
                        <th className="px-3 py-2 text-right font-medium">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {studies.map((study) => {
                        const isSelected = selectedUids.has(study.study_uid)
                        const status = statusMap[study.status]
                        const description = study.study_description?.trim()
                        const patient = study.patient_name?.trim()
                        const patientId = study.patient_id?.trim()
                        const {rel, abs} = formatRelative(study.created_at)

                        return (
                            <tr
                                key={study.study_uid}
                                className={cn(
                                    "border-t transition-colors",
                                    isSelected ? "bg-muted/60" : "hover:bg-muted/40",
                                )}
                            >
                                <td className="px-3 py-2 align-top">
                                    <Checkbox
                                        checked={isSelected}
                                        onChange={() => onToggleSelect(study.study_uid)}
                                        aria-label={`Select study ${study.study_uid}`}
                                    />
                                </td>
                                <td className="px-3 py-2 align-top">
                                    {description
                                        ? <span
                                            className="block truncate font-medium text-foreground"
                                            title={description}
                                        >
                                            {description}
                                        </span>
                                        : <span className="text-muted-foreground">—</span>}
                                </td>
                                <td className="px-3 py-2 align-top">
                                    {patient
                                        ? <span className="block truncate" title={patient}>{patient}</span>
                                        : <span className="text-muted-foreground">—</span>}
                                    {patientId && (
                                        <div
                                            className="truncate text-xs text-muted-foreground"
                                            title={patientId}
                                        >
                                            {patientId}
                                        </div>
                                    )}
                                </td>
                                <td className="px-3 py-2 align-top">
                                    <Badge
                                        variant={status?.variant ?? 'outline'}
                                        title={statusTitle(study)}
                                    >
                                        {status?.label ?? study.status}
                                    </Badge>
                                </td>
                                <td
                                    className="px-3 py-2 align-top whitespace-nowrap overflow-hidden text-ellipsis"
                                    title={abs}
                                >
                                    {rel}
                                </td>
                                <td className="px-3 py-2 align-top overflow-hidden">
                                    <CopyableUid uid={study.study_uid}/>
                                </td>
                                <td className="px-3 py-2 align-top text-right">
                                    <div className="inline-flex items-center gap-1">
                                        {study.status === 'RETRYING' && (
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                onClick={() => onStopRetry(study)}
                                            >
                                                Stop
                                            </Button>
                                        )}
                                        {study.status === 'FAILED' && study.exists && (
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                onClick={() => onRetry(study)}
                                            >
                                                Retry
                                            </Button>
                                        )}
                                        {study.exists && (
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                onClick={() => onSend(study)}
                                            >
                                                Send
                                            </Button>
                                        )}
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => onDelete(study)}
                                        >
                                            Delete
                                        </Button>
                                    </div>
                                </td>
                            </tr>
                        )
                    })}
                </tbody>
            </table>
        </div>
    )
}
