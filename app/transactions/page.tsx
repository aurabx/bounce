'use client';

import {Suspense, useCallback, useEffect, useRef, useState} from 'react'
import {invokeCommand} from "@/app/lib/commands";
import {ArrowPathIcon} from '@heroicons/react/24/outline'
import {classNames} from "@/app/lib/helpers";
import {CurrentUploadAttempts, UploadAttempt, Pagination} from "@/app/lib/types";
import {Button} from "@/app/components/ui/button";
import {Card, CardContent} from "@/app/components/ui/card";
import {Input} from "@/app/components/ui/input";
import TransactionsTable from "@/app/components/TransactionsTable";
import {listen} from "@tauri-apps/api/event";

const DEFAULT_PAGE_SIZE = 25;
const PAGE_SIZE_OPTIONS = [25, 50, 100];
const SEARCH_DEBOUNCE_MS = 300;

// The Transactions view lists the upload-attempt audit trail across all
// studies. Unlike the Studies page (which is driven by a Tauri event into
// Redux because studies update in the background), this is a read-only log
// fetched on demand, so it holds its own page state locally and reads the
// payload straight from the `current_upload_attempts` command.
export default function Page() {
    const [loaded, setLoaded] = useState<boolean>(false);
    const [isFetching, setIsFetching] = useState<boolean>(false);
    const [isRefreshing, setIsRefreshing] = useState<boolean>(false);
    const [currentPage, setCurrentPage] = useState<number>(1);
    const [pageSize, setPageSize] = useState<number>(DEFAULT_PAGE_SIZE);
    const [searchInput, setSearchInput] = useState<string>('');
    const [debouncedSearch, setDebouncedSearch] = useState<string>('');
    const [attempts, setAttempts] = useState<UploadAttempt[]>([]);
    const [pagination, setPagination] = useState<Pagination | null>(null);

    const totalItems = pagination?.total_items ?? 0;
    const totalPages = pagination?.total_pages ?? 0;
    const offset = pagination?.offset ?? 0;
    const itemsOnPage = pagination?.items_on_page ?? 0;
    const startIndex = itemsOnPage > 0 ? offset + 1 : 0;
    const endIndex = offset + itemsOnPage;

    const loadAttempts = useCallback(
        async (page: number, limit: number, search: string) => {
            setIsFetching(true);
            try {
                const result = await invokeCommand('current_upload_attempts', {
                    page,
                    limit,
                    search: search.trim() === '' ? undefined : search.trim(),
                }) as CurrentUploadAttempts;
                setAttempts(result?.attempts ?? []);
                setPagination(result?.pagination ?? null);
            } catch (error) {
                console.error('Error loading transactions:', error);
                setAttempts([]);
                setPagination(null);
            } finally {
                setIsFetching(false);
                setLoaded(true);
            }
        },
        [],
    );

    // Debounce search input with a plain timeout — the search box is local to
    // this page and does not warrant a debounce-hook dependency.
    useEffect(() => {
        const handle = window.setTimeout(() => {
            setDebouncedSearch(searchInput);
        }, SEARCH_DEBOUNCE_MS);

        return () => window.clearTimeout(handle);
    }, [searchInput]);

    // Return to page 1 whenever the filter or page size changes so the
    // operator never lands on an out-of-range page.
    useEffect(() => {
        setCurrentPage(1);
    }, [debouncedSearch, pageSize]);

    useEffect(() => {
        loadAttempts(currentPage, pageSize, debouncedSearch);
    }, [loadAttempts, currentPage, pageSize, debouncedSearch]);

    // Keep the latest paging/search params in a ref so the live-update
    // listener (bound once) can re-fetch the page the user is currently
    // viewing without re-binding on every input change.
    const queryParamsRef = useRef({
        page: currentPage,
        limit: pageSize,
        search: debouncedSearch,
    });
    useEffect(() => {
        queryParamsRef.current = {
            page: currentPage,
            limit: pageSize,
            search: debouncedSearch,
        };
    }, [currentPage, pageSize, debouncedSearch]);

    // Subscribe to backend upload-attempt mutations and refresh the visible
    // page. A short debounce coalesces bursts (claim → success/failure can
    // emit multiple events per upload, and several uploads run concurrently).
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        let cancelled = false;
        let debounceHandle: number | undefined;

        const REFRESH_DEBOUNCE_MS = 250;

        const scheduleRefresh = () => {
            if (debounceHandle !== undefined) {
                window.clearTimeout(debounceHandle);
            }
            debounceHandle = window.setTimeout(() => {
                const params = queryParamsRef.current;
                loadAttempts(params.page, params.limit, params.search);
            }, REFRESH_DEBOUNCE_MS);
        };

        listen('transactions-updated', scheduleRefresh)
            .then((dispose) => {
                if (cancelled) {
                    dispose();
                    return;
                }
                unlisten = dispose;
            })
            .catch((e) => {
                console.error('Failed to subscribe to transactions-updated:', e);
            });

        return () => {
            cancelled = true;
            if (debounceHandle !== undefined) {
                window.clearTimeout(debounceHandle);
            }
            unlisten?.();
        };
    }, [loadAttempts]);

    const refresh = async () => {
        setIsRefreshing(true);
        try {
            await loadAttempts(currentPage, pageSize, debouncedSearch);
        } finally {
            setIsRefreshing(false);
        }
    };

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
    const showEmptySearchResult = loaded && attempts.length === 0 && hasActiveSearch;
    const showEmptyDatabase = loaded && attempts.length === 0 && !hasActiveSearch;
    const tableDim = isFetching && attempts.length > 0;

    return (
        <div className="h-full flex flex-col space-y-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex-1 min-w-[16rem] max-w-md">
                    <Input
                        value={searchInput}
                        onChange={(e) => setSearchInput(e.target.value)}
                        placeholder="Search UID, upload ID, status, or error…"
                        aria-label="Search transactions"
                    />
                </div>

                <div className="flex items-center gap-2">
                    <label className="flex items-center gap-2 text-sm text-muted-foreground">
                        <span>Page size</span>
                        <select
                            value={pageSize}
                            onChange={(e) => setPageSize(Number(e.target.value))}
                            className="h-9 min-w-[5rem] rounded-md border border-input bg-background pl-3 pr-8 text-sm text-foreground"
                            aria-label="Transactions per page"
                        >
                            {PAGE_SIZE_OPTIONS.map((opt) => (
                                <option key={opt} value={opt}>{opt}</option>
                            ))}
                        </select>
                    </label>

                    <Button
                        variant="outline"
                        size="sm"
                        onClick={refresh}
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
                </div>
            </div>

            <div className="text-sm text-muted-foreground">
                {totalItems > 0 ? (
                    <span>
                        Showing {startIndex} to {Math.min(endIndex, totalItems)} of {totalItems} transactions
                    </span>
                ) : null}
            </div>

            <div className="flex-1">
                <Suspense fallback={<Loading/>}>
                    {!loaded ? (
                        <Loading/>
                    ) : showEmptyDatabase ? (
                        <Card>
                            <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                                <div className="text-muted-foreground">
                                    <h3 className="text-lg font-medium mb-2">No transactions yet</h3>
                                    <p className="text-sm">Upload attempts will appear here once studies are sent to Aurabox.</p>
                                </div>
                            </CardContent>
                        </Card>
                    ) : showEmptySearchResult ? (
                        <div className="flex items-center justify-between rounded-md border bg-muted/20 px-4 py-3 text-sm">
                            <span>
                                No transactions match &ldquo;{trimmedSearch}&rdquo;. Clear the search to see all transactions.
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
                                <TransactionsTable attempts={attempts}/>
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
