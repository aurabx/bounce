'use client';

import { useMemo, useState, useEffect, useRef } from 'react'
import { openPath } from '@tauri-apps/plugin-opener'
import { appLogDir, join } from '@tauri-apps/api/path'
import { Button } from '@/app/components/ui/button'
import { Input } from '@/app/components/ui/input'
import { useAppDispatch, useAppSelector } from '@/app/lib/hook'
import { clearLogs, LogLevelName } from '@/app/lib/store'

const MODULE_FILTERS = [
    { label: 'All', value: 'all' },
    { label: 'Receiver', value: 'receiver' },
    { label: 'Transmitter', value: 'transmitter' },
    { label: 'Query', value: 'query' },
    { label: 'Aura', value: 'aura' },
] as const

const LEVEL_FILTERS: { label: string; value: LogLevelName | 'all' }[] = [
    { label: 'All', value: 'all' },
    { label: 'Info', value: 'info' },
    { label: 'Warn', value: 'warn' },
    { label: 'Error', value: 'error' },
]

// Severity ranking used by the level filter — entries at or above the chosen
// level pass.
const LEVEL_RANK: Record<LogLevelName, number> = {
    trace: 0,
    debug: 1,
    info: 2,
    warn: 3,
    error: 4,
}

export default function Page() {
    const dispatch = useAppDispatch()
    const logs = useAppSelector((state) => state.main.logs)

    const [search, setSearch] = useState('')
    const [selectedModule, setSelectedModule] = useState<(typeof MODULE_FILTERS)[number]['value']>('all')
    const [minLevel, setMinLevel] = useState<LogLevelName | 'all'>('all')
    const [wrapLines, setWrapLines] = useState(false) // Default to no-wrap for terminal feel
    const bottomRef = useRef<HTMLDivElement>(null)

    // Auto-scroll on new logs
    useEffect(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'auto' })
    }, [logs.length])

    const filteredLogs = useMemo(() => {
        const minRank = minLevel === 'all' ? -1 : LEVEL_RANK[minLevel]

        return logs.filter((entry) => {
            if (LEVEL_RANK[entry.level] < minRank) {
                return false
            }

            const moduleMatch = selectedModule === 'all'
                ? true
                : entry.module.toLowerCase().includes(selectedModule)

            if (!moduleMatch) {
                return false
            }

            if (search.trim().length === 0) {
                return true
            }

            const haystack = `${entry.level} ${entry.module} ${entry.message}`.toLowerCase()
            return haystack.includes(search.trim().toLowerCase())
        })
    }, [logs, search, selectedModule, minLevel])

    const openLogPath = async () => {
        const logDirPath = await appLogDir();
        const logFilePath = await join(logDirPath, 'logs.log');
        await openPath(logFilePath);
    }

    const getLevelColor = (level: string) => {
        switch (level) {
            case 'error': return 'text-red-400'
            case 'warn': return 'text-amber-400'
            case 'info': return 'text-blue-400'
            case 'debug': return 'text-emerald-400'
            case 'trace': return 'text-slate-500'
            default: return 'text-slate-300'
        }
    }

    return (
        <div className="flex h-full flex-col bg-background rounded-md border shadow-sm overflow-hidden">
            {/* Compact Toolbar */}
            <div className="flex flex-col gap-2 border-b bg-card p-2 shadow-sm z-10">
                <div className="flex flex-wrap items-center gap-2">
                    <Input
                        value={search}
                        onChange={(event) => setSearch(event.target.value)}
                        placeholder="Search logs..."
                        className="h-8 w-full sm:w-64 text-xs bg-background"
                    />

                    <div className="flex h-8 items-center rounded-md border bg-muted p-1">
                        {MODULE_FILTERS.map((filter) => (
                            <button
                                key={filter.value}
                                onClick={() => setSelectedModule(filter.value)}
                                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
                                    selectedModule === filter.value
                                        ? 'bg-background text-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-foreground'
                                }`}
                            >
                                {filter.label}
                            </button>
                        ))}
                    </div>

                    <div className="flex h-8 items-center rounded-md border bg-muted p-1">
                        {LEVEL_FILTERS.map((filter) => (
                            <button
                                key={filter.value}
                                onClick={() => setMinLevel(filter.value)}
                                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
                                    minLevel === filter.value
                                        ? 'bg-background text-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-foreground'
                                }`}
                            >
                                {filter.label}
                            </button>
                        ))}
                    </div>

                    <div className="flex-1" />

                    <div className="flex items-center gap-1">
                        <Button
                            variant={wrapLines ? 'secondary' : 'ghost'}
                            size="sm"
                            className="h-8 px-3 text-xs"
                            onClick={() => setWrapLines(!wrapLines)}
                        >
                            Wrap
                        </Button>
                        <div className="mx-1 h-4 w-px bg-border hidden sm:block" />
                        <Button
                            variant="ghost"
                            size="sm"
                            className="h-8 px-3 text-xs text-muted-foreground hover:text-destructive hidden sm:flex"
                            onClick={() => dispatch(clearLogs())}
                        >
                            Clear
                        </Button>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="h-8 px-3 text-xs hidden sm:flex"
                            onClick={openLogPath}
                        >
                            File
                        </Button>
                    </div>
                </div>
            </div>

            {/* Terminal View */}
            <div className="flex-1 overflow-auto bg-[#1e1e1e] p-2 text-[11px] leading-relaxed text-[#d4d4d4] antialiased selection:bg-blue-900 font-mono">
                {filteredLogs.length === 0 ? (
                    <div className="p-2 text-slate-500 italic">No logs match the current filters.</div>
                ) : (
                    <div className="flex flex-col">
                        {filteredLogs.map((entry) => (
                            <div
                                key={entry.id}
                                className={`flex gap-3 px-2 py-[2px] hover:bg-white/5 ${
                                    wrapLines ? 'whitespace-pre-wrap break-words' : 'whitespace-pre w-max min-w-full'
                                }`}
                            >
                                <span className="shrink-0 select-none text-slate-500">
                                    {new Date(entry.timestamp).toLocaleTimeString(undefined, {
                                        hour12: false,
                                        hour: '2-digit',
                                        minute: '2-digit',
                                        second: '2-digit',
                                    })}
                                </span>
                                <span className={`shrink-0 w-10 uppercase select-none font-semibold ${getLevelColor(entry.level)}`}>
                                    {entry.level}
                                </span>
                                <span className="shrink-0 w-20 select-none text-slate-400 truncate" title={entry.module}>
                                    {entry.module}
                                </span>
                                <span className="flex-1">{entry.message}</span>
                            </div>
                        ))}
                        <div ref={bottomRef} className="h-1" />
                    </div>
                )}
            </div>
        </div>
    )
}
