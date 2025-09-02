'use client';

import {Suspense, useEffect, useState} from 'react'
import {useAppSelector} from "@/app/lib/hook";
import {invoke} from "@tauri-apps/api/core";
import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react'
import { ChevronLeftIcon, ChevronRightIcon, EllipsisVerticalIcon, ArrowPathIcon } from '@heroicons/react/24/outline'
import {classNames, formatDicomDateAndTime} from "@/app/lib/helpers";
import {Study} from "@/app/lib/types";

const statuses = {
    COMPLETE: 'text-green-700 bg-green-50 ring-green-600/20',
    SENT: 'text-green-700 bg-green-50 ring-green-600/20',
    "IN-PROGRESS": 'text-yellow-600 bg-yellow-50 ring-yellow-500/10',
    ARCHIVED: 'text-blue-800 bg-blue-50 ring-blue-600/20',
    UNKNOWN: 'text-gray-800 bg-gray-50 ring-gray-600/20',
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

    const loadStudies = async (page: number = currentPage, limit: number = ITEMS_PER_PAGE) => {
        setLoaded(false);
        try {
            await invoke('current_studies', { page, limit });
        } catch (error) {
            console.error('Error loading studies:', error);
        } finally {
            setLoaded(true);
        }
    };

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
    }, []);

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
        <div className="h-full flex flex-col">
            {/* Header with refresh button */}
            <div className="mb-6 flex justify-between items-center">
                <div className="text-sm text-gray-600">
                    {totalItems > 0 && (
                        <span>
                            Showing {startIndex + 1} to {Math.min(endIndex, totalItems)} of {totalItems} studies
                        </span>
                    )}
                </div>
                <button
                    onClick={refreshStudies}
                    disabled={isRefreshing}
                    className={classNames(
                        "inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md border shadow-sm",
                        isRefreshing
                            ? "border-gray-300 text-gray-400 bg-gray-50 cursor-not-allowed"
                            : "border-gray-300 text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                    )}
                >
                    <ArrowPathIcon
                        className={classNames(
                            "h-4 w-4",
                            isRefreshing ? "animate-spin" : ""
                        )}
                    />
                    {isRefreshing ? 'Refreshing...' : 'Refresh'}
                </button>
            </div>

            <div className="flex-1">
                <Suspense fallback={<Loading/>}>
                    {loaded ? (
                        <div className="h-full">
                            {currentStudies.length === 0 ? (
                                <div className="bg-white shadow-lg rounded-lg p-12 w-full text-center">
                                    <div className="text-gray-500">
                                        <h3 className="text-lg font-medium mb-2">No studies found</h3>
                                        <p className="text-sm">Studies will appear here once they are received via DICOM.</p>
                                    </div>
                                </div>
                            ) : (
                                <>
                                    <ul role="list" className="divide-y divide-gray-100 space-y-2 mb-6">
                                        {currentStudies.map((study) => (
                                            <li key={study.study_uid} className="flex items-center justify-between gap-x-6 py-5 bg-white shadow-lg rounded-lg p-6 w-full flex-grow">
                                                <div className="min-w-0">
                                                    <div className="flex items-start gap-x-3">
                                                        <p className="text-sm/6 font-semibold text-gray-900">{study.study_description}</p>
                                                        <p
                                                            className={classNames(
                                                                statuses.hasOwnProperty(study.status) ? statuses[study.status as keyof typeof statuses] : statuses.UNKNOWN,
                                                                'mt-0.5 rounded-md px-1.5 py-0.5 text-xs font-medium whitespace-nowrap ring-1 ring-inset',
                                                            )}
                                                        >
                                                            {study.status}
                                                        </p>
                                                    </div>
                                                    <div className="mt-1 flex flex-col items-start gap-x-2 text-xs/5 text-gray-500">
                                                        <p className="whitespace-nowrap truncate w-full">
                                                            Study Instance UID: {study.study_uid}
                                                        </p>
                                                        <p className="truncate w-full">Created at: {formatDicomDateAndTime(study.study_date, study.study_time)}</p>
                                                    </div>
                                                </div>
                                                <div className="flex flex-none items-center gap-x-4">
                                                    {study.exists && (
                                                        <a
                                                            href="#"
                                                            onClick={() => sendStudy(study)}
                                                            className="hidden rounded-md bg-white px-2.5 py-1.5 text-sm font-semibold text-gray-900 ring-1 shadow-xs ring-gray-300 ring-inset hover:bg-gray-50 sm:block"
                                                        >
                                                            Send study <span className="sr-only">, {study.study_description}</span>
                                                        </a>
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
                                            </li>
                                        ))}
                                    </ul>

                                    {/* Pagination */}
                                    {totalPages > 1 && (
                                        <div className="bg-white px-4 py-3 flex items-center justify-between border-t border-gray-200 sm:px-6 rounded-lg shadow-lg">
                                            <div className="flex-1 flex justify-between sm:hidden">
                                                <button
                                                    onClick={goToPrevious}
                                                    disabled={currentPage === 1}
                                                    className={classNames(
                                                        "relative inline-flex items-center px-4 py-2 text-sm font-medium rounded-md",
                                                        currentPage === 1
                                                            ? "text-gray-400 bg-gray-50 cursor-not-allowed"
                                                            : "text-gray-700 bg-white hover:bg-gray-50 border border-gray-300"
                                                    )}
                                                >
                                                    Previous
                                                </button>
                                                <button
                                                    onClick={goToNext}
                                                    disabled={currentPage === totalPages}
                                                    className={classNames(
                                                        "ml-3 relative inline-flex items-center px-4 py-2 text-sm font-medium rounded-md",
                                                        currentPage === totalPages
                                                            ? "text-gray-400 bg-gray-50 cursor-not-allowed"
                                                            : "text-gray-700 bg-white hover:bg-gray-50 border border-gray-300"
                                                    )}
                                                >
                                                    Next
                                                </button>
                                            </div>
                                            <div className="hidden sm:flex-1 sm:flex sm:items-center sm:justify-between">
                                                <div>
                                                    <p className="text-sm text-gray-700">
                                                        Showing <span className="font-medium">{startIndex + 1}</span> to{' '}
                                                        <span className="font-medium">{Math.min(endIndex, totalItems)}</span> of{' '}
                                                        <span className="font-medium">{totalItems}</span> results
                                                    </p>
                                                </div>
                                                <div>
                                                    <nav className="relative z-0 inline-flex rounded-md shadow-sm -space-x-px" aria-label="Pagination">
                                                        <button
                                                            onClick={goToPrevious}
                                                            disabled={currentPage === 1}
                                                            className={classNames(
                                                                "relative inline-flex items-center px-2 py-2 rounded-l-md text-sm font-medium",
                                                                currentPage === 1
                                                                    ? "text-gray-300 bg-gray-50 cursor-not-allowed"
                                                                    : "text-gray-500 bg-white hover:bg-gray-50 border border-gray-300"
                                                            )}
                                                        >
                                                            <span className="sr-only">Previous</span>
                                                            <ChevronLeftIcon className="h-5 w-5" aria-hidden="true" />
                                                        </button>

                                                        {getPageNumbers().map((page, index) => (
                                                            <span key={index}>
                                                                {page === '...' ? (
                                                                    <span className="relative inline-flex items-center px-4 py-2 border border-gray-300 bg-white text-sm font-medium text-gray-700">
                                                                        ...
                                                                    </span>
                                                                ) : (
                                                                    <button
                                                                        onClick={() => goToPage(page as number)}
                                                                        className={classNames(
                                                                            "relative inline-flex items-center px-4 py-2 text-sm font-medium border",
                                                                            currentPage === page
                                                                                ? "z-10 bg-indigo-50 border-indigo-500 text-indigo-600"
                                                                                : "bg-white border-gray-300 text-gray-500 hover:bg-gray-50"
                                                                        )}
                                                                    >
                                                                        {page}
                                                                    </button>
                                                                )}
                                                            </span>
                                                        ))}

                                                        <button
                                                            onClick={goToNext}
                                                            disabled={currentPage === totalPages}
                                                            className={classNames(
                                                                "relative inline-flex items-center px-2 py-2 rounded-r-md text-sm font-medium",
                                                                currentPage === totalPages
                                                                    ? "text-gray-300 bg-gray-50 cursor-not-allowed"
                                                                    : "text-gray-500 bg-white hover:bg-gray-50 border border-gray-300"
                                                            )}
                                                        >
                                                            <span className="sr-only">Next</span>
                                                            <ChevronRightIcon className="h-5 w-5" aria-hidden="true" />
                                                        </button>
                                                    </nav>
                                                </div>
                                            </div>
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
    return <h2>🌀 Loading...</h2>;
}