'use client';

import {Suspense, useEffect, useState, useCallback} from 'react'
import {useAppSelector} from "@/app/lib/hook";
import {invoke} from "@tauri-apps/api/core";
import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react'
import { ChevronLeftIcon, ChevronRightIcon, EllipsisVerticalIcon, ArrowPathIcon } from '@heroicons/react/24/outline'
import {classNames, formatDicomDateAndTime} from "@/app/lib/helpers";
import {Study} from "@/app/lib/types";
import { Button } from "@/app/components/ui/button";
import { Card, CardContent } from "@/app/components/ui/card";
import { Badge } from "@/app/components/ui/badge";

const statusMap: Record<string, { variant: "default" | "secondary" | "destructive" | "outline" | "success" | "warning", label: string }> = {
    COMPLETE: { variant: 'success', label: 'Complete' },
    SENT: { variant: 'success', label: 'Sent' },
    "IN-PROGRESS": { variant: 'warning', label: 'In Progress' },
    ARCHIVED: { variant: 'secondary', label: 'Archived' },
    UNKNOWN: { variant: 'outline', label: 'Unknown' },
}

const ITEMS_PER_PAGE = 10;

export default function Page() {
    const [loaded, setLoaded] = useState<boolean>(false);
    const [currentPage, setCurrentPage] = useState<number>(1);
    const [isRefreshing, setIsRefreshing] = useState<boolean>(false);

    const studies = useAppSelector((state) => state.main.studies)

    // Calculate pagination - now these will need to come from backend response
    // For now, keeping client-side calculations but these should ideally come from the API response
    const totalItems = studies.length;
    const totalPages = Math.ceil(totalItems / ITEMS_PER_PAGE);
    const startIndex = (currentPage - 1) * ITEMS_PER_PAGE;
    const endIndex = startIndex + ITEMS_PER_PAGE;
    const currentStudies = studies; // Backend should return paginated results

    const loadStudies = useCallback(async (page: number = currentPage, limit: number = ITEMS_PER_PAGE) => {
        setLoaded(false);
        try {
            await invoke('current_studies', { page, limit });
        } catch (error) {
            console.error('Error loading studies:', error);
        } finally {
            setLoaded(true);
        }
    }, [currentPage]);

    const refreshStudies = async () => {
        setIsRefreshing(true);
        try {
            await loadStudies(currentPage, ITEMS_PER_PAGE);
        } finally {
            setIsRefreshing(false);
        }
    };

    useEffect(() => {
        loadStudies(1, ITEMS_PER_PAGE);
    }, [loadStudies]);

    // Reset to first page when studies change
    useEffect(() => {
        if (currentPage > totalPages && totalPages > 0) {
            setCurrentPage(1);
        }
    }, [studies, currentPage, totalPages]);

    const sendStudy = async (study: Study) => {
        await invoke('send_study', {
            studyUid: study.study_uid,
        });
    };

    const deleteStudy = async (study: Study) => {
        setLoaded(false)

        await invoke('delete_study', {
            studyUid: study.study_uid,
        });

        await loadStudies(currentPage, ITEMS_PER_PAGE);
    };

    const goToPage = (page: number) => {
        const newPage = Math.max(1, Math.min(page, totalPages));
        setCurrentPage(newPage);
        loadStudies(newPage, ITEMS_PER_PAGE);
    };

    const goToPrevious = () => {
        goToPage(currentPage - 1);
    };

    const goToNext = () => {
        goToPage(currentPage + 1);
    };

    // Generate page numbers for pagination
    const getPageNumbers = () => {
        const pages = [];
        const maxPagesToShow = 5;

        if (totalPages <= maxPagesToShow) {
            // Show all pages if total is small
            for (let i = 1; i <= totalPages; i++) {
                pages.push(i);
            }
        } else {
            // Show smart pagination with ellipsis
            if (currentPage <= 3) {
                pages.push(1, 2, 3, 4, '...', totalPages);
            } else if (currentPage >= totalPages - 2) {
                pages.push(1, '...', totalPages - 3, totalPages - 2, totalPages - 1, totalPages);
            } else {
                pages.push(1, '...', currentPage - 1, currentPage, currentPage + 1, '...', totalPages);
            }
        }

        return pages;
    };

    return (
        <div className="h-full flex flex-col space-y-6">
            {/* Header with refresh button */}
            <div className="flex justify-between items-center">
                <div className="text-sm text-muted-foreground">
                    {totalItems > 0 && (
                        <span>
                            Showing {startIndex + 1} to {Math.min(endIndex, totalItems)} of {totalItems} studies
                        </span>
                    )}
                </div>
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
                            isRefreshing ? "animate-spin" : ""
                        )}
                    />
                    {isRefreshing ? 'Refreshing...' : 'Refresh'}
                </Button>
            </div>

            <div className="flex-1">
                <Suspense fallback={<Loading/>}>
                    {loaded ? (
                        <div className="space-y-4">
                            {currentStudies.length === 0 ? (
                                <Card>
                                    <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                                        <div className="text-muted-foreground">
                                            <h3 className="text-lg font-medium mb-2">No studies found</h3>
                                            <p className="text-sm">Studies will appear here once they are received via DICOM.</p>
                                        </div>
                                    </CardContent>
                                </Card>
                            ) : (
                                <>
                                    <div className="space-y-4">
                                        {currentStudies.map((study) => (
                                            <Card key={study.study_uid}>
                                                <CardContent className="p-6 flex items-center justify-between gap-x-6">
                                                <div className="min-w-0">
                                                    <div className="flex items-center gap-x-3 mb-1">
                                                        <p className="text-sm font-semibold text-foreground">{study.study_description}</p>
                                                        <Badge variant={statusMap[study.status]?.variant || 'outline'}>
                                                            {study.status}
                                                        </Badge>
                                                    </div>
                                                    <div className="flex flex-col items-start gap-x-2 text-xs text-muted-foreground">
                                                        <p className="whitespace-nowrap truncate w-full">
                                                            UID: {study.study_uid}
                                                        </p>
                                                        <p className="truncate w-full">Created: {formatDicomDateAndTime(study.study_date, study.study_time)}</p>
                                                    </div>
                                                </div>
                                                <div className="flex flex-none items-center gap-x-4">
                                                    {study.exists && (
                                                        <Button
                                                            variant="outline"
                                                            size="sm"
                                                            onClick={() => sendStudy(study)}
                                                            className="hidden sm:flex"
                                                        >
                                                            Send study
                                                        </Button>
                                                    )}

                                                    <Menu as="div" className="relative flex-none">
                                                        <MenuButton className="-m-2.5 block p-2.5 text-gray-500 hover:text-gray-900">
                                                            <span className="sr-only">Open options</span>
                                                            <EllipsisVerticalIcon aria-hidden="true" className="size-5" />
                                                        </MenuButton>
                                                        <MenuItems
                                                            transition
                                                            className="absolute right-0 z-10 mt-2 w-32 origin-top-right rounded-md bg-white py-2 ring-1 shadow-lg ring-gray-900/5 transition focus:outline-hidden data-closed:scale-95 data-closed:transform data-closed:opacity-0 data-enter:duration-100 data-enter:ease-out data-leave:duration-75 data-leave:ease-in"
                                                        >
                                                            <MenuItem>
                                                                <a
                                                                    href="#"
                                                                    onClick={() => deleteStudy(study)}
                                                                    className="block px-3 py-1 text-sm/6 text-gray-900 data-focus:bg-gray-50 data-focus:outline-hidden"
                                                                >
                                                                    Delete<span className="sr-only">, {study.study_description}</span>
                                                                </a>
                                                            </MenuItem>
                                                        </MenuItems>
                                                    </Menu>
                                                </div>
                                                </CardContent>
                                            </Card>
                                        ))}
                                    </div>

                                    {/* Pagination */}
                                    {totalPages > 1 && (
                                        <div className="flex items-center justify-center space-x-2 py-4">
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
                                                    <span key={index}>
                                                        {page === '...' ? (
                                                            <span className="px-4 py-2 text-sm text-muted-foreground">...</span>
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
                                </>
                            )}
                        </div>
                    ) : null}
                </Suspense>
            </div>
        </div>
    );
}

function Loading() {
    return <h2>Loading...</h2>;
}
