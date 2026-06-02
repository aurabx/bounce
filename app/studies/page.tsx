'use client';

import {Suspense, useEffect, useState, useCallback, useMemo} from 'react'
import {useAppSelector} from "@/app/lib/hook";
import {invokeCommand} from "@/app/lib/commands";
import {listen} from "@tauri-apps/api/event";
import {confirm} from "@tauri-apps/plugin-dialog";
import {ArrowPathIcon} from '@heroicons/react/24/outline'
import {classNames} from "@/app/lib/helpers";
import {Study} from "@/app/lib/types";
import {Button} from "@/app/components/ui/button";
import {Card, CardContent} from "@/app/components/ui/card";
import {Input} from "@/app/components/ui/input";
import StudiesTable from "@/app/components/StudiesTable";

const DEFAULT_PAGE_SIZE = 25;
const PAGE_SIZE_OPTIONS = [25, 50, 100];
const SEARCH_DEBOUNCE_MS = 300;

export default function Page() {
    const [loaded, setLoaded] = useState<boolean>(false);
    const [isFetching, setIsFetching] = useState<boolean>(false);
    const [isRefreshing, setIsRefreshing] = useState<boolean>(false);
    const [currentPage, setCurrentPage] = useState<number>(1);
    const [pageSize, setPageSize] = useState<number>(DEFAULT_PAGE_SIZE);
    const [searchInput, setSearchInput] = useState<string>('');
    const [debouncedSearch, setDebouncedSearch] = useState<string>('');
    const [selectedUids, setSelectedUids] = useState<Set<string>>(new Set());
    const [pendingRetryUids, setPendingRetryUids] = useState<Set<string>>(new Set());
    const [diskWarning, setDiskWarning] = useState<{availableBytes: number, critical: boolean} | null>(null);

    const studies = useAppSelector((state) => state.main.studies);
    const pagination = useAppSelector((state) => state.main.pagination);

    const totalItems = pagination?.total_items ?? 0;
    const totalPages = pagination?.total_pages ?? 0;
    const offset = pagination?.offset ?? 0;
    const itemsOnPage = pagination?.items_on_page ?? 0;
    const startIndex = itemsOnPage > 0 ? offset + 1 : 0;
    const endIndex = offset + itemsOnPage;

    const loadStudies = useCallback(
        async (page: number, limit: number, search: string) => {
            setIsFetching(true);
            try {
                await invokeCommand('current_studies', {
                    page,
                    limit,
                    search: search.trim() === '' ? undefined : search.trim(),
                });
            } catch (error) {
                console.error('Error loading studies:', error);
            } finally {
                setIsFetching(false);
                setLoaded(true);
            }
        },
        [],
    );

    // Debounce search input. We do this with setTimeout rather than a
    // library dependency: the search box is local to this page and we
    // do not need the broader semantics of a debounce hook.
    useEffect(() => {
        const handle = window.setTimeout(() => {
            setDebouncedSearch(searchInput);
        }, SEARCH_DEBOUNCE_MS);

        return () => window.clearTimeout(handle);
    }, [searchInput]);

    // When the filter changes, return to page 1 before fetching so the
    // operator never sees "page 5 of 1" or an empty page during typing.
    useEffect(() => {
        setCurrentPage(1);
    }, [debouncedSearch, pageSize]);

    // Selection is per-page (see Decision Log). Clear whenever the
    // visible window changes so the action bar never claims rows the
    // operator can no longer see.
    useEffect(() => {
        setSelectedUids(new Set());
    }, [currentPage, pageSize, debouncedSearch]);

    useEffect(() => {
        loadStudies(currentPage, pageSize, debouncedSearch);
    }, [loadStudies, currentPage, pageSize, debouncedSearch]);

    const refreshStudies = async () => {
        setIsRefreshing(true);
        try {
            await loadStudies(currentPage, pageSize, debouncedSearch);
        } finally {
            setIsRefreshing(false);
        }
    };

    // Surface low-disk warnings emitted by the backend (receiver write path
    // and the upload scheduler) as a dismissible banner.
    useEffect(() => {
        const unlisten = listen<{available_bytes: number, critical: boolean}>(
            'disk-warning',
            (event) => {
                setDiskWarning({
                    availableBytes: event.payload.available_bytes,
                    critical: event.payload.critical,
                });
            },
        );
        return () => {
            unlisten.then((fn) => fn());
        };
    }, []);

    const sendStudy = async (study: Study) => {
        await invokeCommand('send_study', {studyUid: study.study_uid});
    };

    const retryStudy = async (study: Study) => {
        const uid = study.study_uid;
        // Mark the row as in-flight so the Retry button can render a
        // disabled "Retrying…" spinner — without this the click feels
        // like it has done nothing while the backend reclaims the study
        // and the list reloads.
        setPendingRetryUids((prev) => {
            if (prev.has(uid)) return prev;
            const next = new Set(prev);
            next.add(uid);
            return next;
        });
        try {
            await invokeCommand('retry_study', {studyUid: uid});
            await loadStudies(currentPage, pageSize, debouncedSearch);
        } finally {
            setPendingRetryUids((prev) => {
                if (!prev.has(uid)) return prev;
                const next = new Set(prev);
                next.delete(uid);
                return next;
            });
        }
    };

    const deleteStudy = async (study: Study) => {
        const ok = await confirm(
            `Delete study ${study.study_description || study.study_uid}? This removes it from disk and cannot be undone.`,
            {title: 'Delete study', kind: 'warning'},
        );
        if (!ok) return;

        await invokeCommand('delete_study', {studyUid: study.study_uid});
        await loadStudies(currentPage, pageSize, debouncedSearch);
    };

    const onBulkSend = async () => {
        const uids = Array.from(selectedUids);
        if (uids.length === 0) return;

        const ok = await confirm(
            `Send ${uids.length} ${uids.length === 1 ? 'study' : 'studies'} to Aurabox?`,
            {title: 'Send selected studies', kind: 'info'},
        );
        if (!ok) return;

        await invokeCommand('bulk_send_studies', {studyUids: uids});
        setSelectedUids(new Set());
        await loadStudies(currentPage, pageSize, debouncedSearch);
    };

    const onBulkDelete = async () => {
        const uids = Array.from(selectedUids);
        if (uids.length === 0) return;

        const ok = await confirm(
            `Delete ${uids.length} ${uids.length === 1 ? 'study' : 'studies'}? This removes them from disk and cannot be undone.`,
            {title: 'Delete selected studies', kind: 'warning'},
        );
        if (!ok) return;

        await invokeCommand('bulk_delete_studies', {studyUids: uids});
        setSelectedUids(new Set());
        await loadStudies(currentPage, pageSize, debouncedSearch);
    };

    const onDeleteAll = async () => {
        const ok = await confirm(
            `Delete all ${totalItems} ${totalItems === 1 ? 'study' : 'studies'} and their files? This cannot be undone.`,
            {title: 'Delete all studies', kind: 'warning'},
        );
        if (!ok) return;

        await invokeCommand('delete_all_studies');
        setSelectedUids(new Set());
        setCurrentPage(1);
        await loadStudies(1, pageSize, debouncedSearch);
    };

    const toggleSelect = useCallback((uid: string) => {
        setSelectedUids((prev) => {
            const next = new Set(prev);
            if (next.has(uid)) {
                next.delete(uid);
            } else {
                next.add(uid);
            }
            return next;
        });
    }, []);

    const visibleUids = useMemo(
        () => studies.map((s) => s.study_uid),
        [studies],
    );

    const selectedOnPageCount = useMemo(
        () => visibleUids.filter((uid) => selectedUids.has(uid)).length,
        [visibleUids, selectedUids],
    );

    const allOnPageSelected = visibleUids.length > 0
        && selectedOnPageCount === visibleUids.length;
    const someOnPageSelected = selectedOnPageCount > 0 && !allOnPageSelected;

    const toggleSelectAll = useCallback(() => {
        setSelectedUids((prev) => {
            if (visibleUids.length > 0
                && visibleUids.every((uid) => prev.has(uid))) {
                const next = new Set(prev);
                visibleUids.forEach((uid) => next.delete(uid));
                return next;
            }

            const next = new Set(prev);
            visibleUids.forEach((uid) => next.add(uid));
            return next;
        });
    }, [visibleUids]);

    const goToPage = (page: number) => {
        const target = Math.max(1, Math.min(page, Math.max(totalPages, 1)));
        setCurrentPage(target);
    };

    const goToPrevious = () => goToPage(currentPage - 1);
    const goToNext = () => goToPage(currentPage + 1);

    const getPageNumbers = () => {
        const pages: (number | '...')[] = [];
        const maxPagesToShow = 5;

        if (totalPages <= maxPagesToShow) {
            for (let i = 1; i <= totalPages; i++) pages.push(i);
        } else if (currentPage <= 3) {
            pages.push(1, 2, 3, 4, '...', totalPages);
        } else if (currentPage >= totalPages - 2) {
            pages.push(1, '...', totalPages - 3, totalPages - 2, totalPages - 1, totalPages);
        } else {
            pages.push(1, '...', currentPage - 1, currentPage, currentPage + 1, '...', totalPages);
        }

        return pages;
    };

    const trimmedSearch = debouncedSearch.trim();
    const hasActiveSearch = trimmedSearch.length > 0;
    const showEmptySearchResult = loaded && studies.length === 0 && hasActiveSearch;
    const showEmptyDatabase = loaded && studies.length === 0 && !hasActiveSearch;
    const tableDim = isFetching && studies.length > 0;

    const diskWarningMb = diskWarning
        ? Math.round(diskWarning.availableBytes / (1024 * 1024))
        : 0;

    return (
        <div className="h-full flex flex-col space-y-4">
            {diskWarning && (
                <div
                    className={classNames(
                        "flex items-center justify-between rounded-md border px-4 py-2 text-sm",
                        diskWarning.critical
                            ? "border-destructive/50 bg-destructive/10 text-destructive"
                            : "border-amber-500/50 bg-amber-50 text-amber-700",
                    )}
                >
                    <span>
                        {diskWarning.critical
                            ? `Critically low disk space: ${diskWarningMb} MB free. New uploads are paused until space is freed.`
                            : `Low disk space: ${diskWarningMb} MB free.`}
                    </span>
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setDiskWarning(null)}
                    >
                        Dismiss
                    </Button>
                </div>
            )}

            <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex-1 min-w-[16rem] max-w-md">
                    <Input
                        value={searchInput}
                        onChange={(e) => setSearchInput(e.target.value)}
                        placeholder="Search description, patient, accession, or UID…"
                        aria-label="Search studies"
                    />
                </div>

                <div className="flex items-center gap-2">
                    <label className="flex items-center gap-2 text-sm text-muted-foreground">
                        <span>Page size</span>
                        <select
                            value={pageSize}
                            onChange={(e) => setPageSize(Number(e.target.value))}
                            className="h-9 min-w-[5rem] rounded-md border border-input bg-background pl-3 pr-8 text-sm text-foreground"
                            aria-label="Studies per page"
                        >
                            {PAGE_SIZE_OPTIONS.map((opt) => (
                                <option key={opt} value={opt}>{opt}</option>
                            ))}
                        </select>
                    </label>

                    <Button
                        variant="outline"
                        size="sm"
                        onClick={refreshStudies}
                        disabled={isRefreshing}
                        className="gap-2"
                    >
                        <ArrowPathIcon
                            className={classNames(
                                "h-4 w-4",
                                isRefreshing ? "animate-spin" : "",
                            )}
                        />
                        {isRefreshing ? 'Refreshing…' : 'Refresh'}
                    </Button>

                    {totalItems > 0 && (
                        <Button
                            variant="destructive"
                            size="sm"
                            onClick={onDeleteAll}
                        >
                            Delete all
                        </Button>
                    )}
                </div>
            </div>

            <div className="text-sm text-muted-foreground">
                {totalItems > 0 ? (
                    <span>
                        Showing {startIndex} to {Math.min(endIndex, totalItems)} of {totalItems} studies
                    </span>
                ) : null}
            </div>

            {selectedUids.size > 0 && (
                <div className="flex items-center justify-between rounded-md border bg-muted/30 px-4 py-2">
                    <span className="text-sm">{selectedUids.size} selected</span>
                    <div className="flex gap-2">
                        <Button size="sm" variant="outline" onClick={onBulkSend}>
                            Send selected
                        </Button>
                        <Button size="sm" variant="destructive" onClick={onBulkDelete}>
                            Delete selected
                        </Button>
                    </div>
                </div>
            )}

            <div className="flex-1">
                <Suspense fallback={<Loading/>}>
                    {!loaded ? (
                        <Loading/>
                    ) : showEmptyDatabase ? (
                        <Card>
                            <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                                <div className="text-muted-foreground">
                                    <h3 className="text-lg font-medium mb-2">No studies found</h3>
                                    <p className="text-sm">Studies will appear here once they are received via DICOM.</p>
                                </div>
                            </CardContent>
                        </Card>
                    ) : showEmptySearchResult ? (
                        <div className="flex items-center justify-between rounded-md border bg-muted/20 px-4 py-3 text-sm">
                            <span>
                                No studies match &ldquo;{trimmedSearch}&rdquo;. Clear the search to see all studies.
                            </span>
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={() => setSearchInput('')}
                            >
                                Clear
                            </Button>
                        </div>
                    ) : (
                        <div className="space-y-4">
                            <div
                                className={classNames(
                                    "transition-opacity",
                                    tableDim ? "opacity-60 pointer-events-none" : "",
                                )}
                            >
                                <StudiesTable
                                    studies={studies}
                                    selectedUids={selectedUids}
                                    pendingRetryUids={pendingRetryUids}
                                    onToggleSelect={toggleSelect}
                                    onToggleSelectAll={toggleSelectAll}
                                    allOnPageSelected={allOnPageSelected}
                                    someOnPageSelected={someOnPageSelected}
                                    onSend={sendStudy}
                                    onRetry={retryStudy}
                                    onDelete={deleteStudy}
                                />
                            </div>

                            {totalPages > 1 && (
                                <div className="flex items-center justify-center space-x-2 py-2">
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={goToPrevious}
                                        disabled={currentPage === 1}
                                    >
                                        Previous
                                    </Button>

                                    <div className="flex items-center space-x-2">
                                        {getPageNumbers().map((page, index) => (
                                            <span key={`${page}-${index}`}>
                                                {page === '...' ? (
                                                    <span className="px-4 py-2 text-sm text-muted-foreground">…</span>
                                                ) : (
                                                    <Button
                                                        variant={currentPage === page ? "default" : "outline"}
                                                        size="sm"
                                                        onClick={() => goToPage(page as number)}
                                                    >
                                                        {page}
                                                    </Button>
                                                )}
                                            </span>
                                        ))}
                                    </div>

                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={goToNext}
                                        disabled={currentPage === totalPages}
                                    >
                                        Next
                                    </Button>
                                </div>
                            )}
                        </div>
                    )}
                </Suspense>
            </div>
        </div>
    );
}

function Loading() {
    return <h2>Loading...</h2>;
}
