'use client'

import { useEffect, useState } from 'react'
import { invokeCommand } from '@/app/lib/commands'
import { useAppDispatch, useAppSelector } from '@/app/lib/hook'
import { setDicomServices } from '@/app/lib/store'
import { DicomService, EchoResult } from '@/app/lib/types'
import { Card, CardContent } from '@/app/components/ui/card'
import { Badge } from '@/app/components/ui/badge'
import { Button } from '@/app/components/ui/button'
import { ArrowPathIcon, CheckCircleIcon, XCircleIcon } from '@heroicons/react/24/outline'
import { classNames } from '@/app/lib/helpers'

type EchoState = {
    state: 'idle' | 'running' | 'ok' | 'failed',
    latencyMs?: number,
    error?: string,
}

export default function PacsServices() {
    const dispatch = useAppDispatch()
    const services = useAppSelector((state) => state.main.dicomServices)
    const [refreshing, setRefreshing] = useState(false)
    const [error, setError] = useState<string | null>(null)
    const [echoes, setEchoes] = useState<Record<string, EchoState>>({})

    useEffect(() => {
        loadFromCache()
    }, [])

    const loadFromCache = async () => {
        try {
            const cached = await invokeCommand('list_pacs_services')
            dispatch(setDicomServices(cached))
        } catch (err) {
            console.error('Failed to load cached PACS services:', err)
        }
    }

    const refresh = async () => {
        setRefreshing(true)
        setError(null)
        try {
            const fresh = await invokeCommand('refresh_pacs_services')
            dispatch(setDicomServices(fresh))
        } catch (err) {
            setError(String(err))
        } finally {
            setRefreshing(false)
        }
    }

    const echo = async (service: DicomService) => {
        setEchoes((prev) => ({
            ...prev,
            [service.id]: { state: 'running' },
        }))

        try {
            const result = await invokeCommand('echo_pacs_service', {
                serviceId: service.id,
            })
            setEchoes((prev) => ({
                ...prev,
                [service.id]: result.ok
                    ? { state: 'ok', latencyMs: result.latency_ms ?? undefined }
                    : { state: 'failed', error: result.error ?? 'Unknown error' },
            }))
        } catch (err) {
            setEchoes((prev) => ({
                ...prev,
                [service.id]: { state: 'failed', error: String(err) },
            }))
        }
    }

    return (
        <div className="space-y-4">
            <div className="flex justify-between items-center">
                <div className="text-sm text-muted-foreground">
                    {services.length > 0
                        ? `${services.length} configured PACS ${services.length === 1 ? 'service' : 'services'}`
                        : 'No PACS services cached yet'}
                </div>
                <Button
                    variant="outline"
                    size="sm"
                    onClick={refresh}
                    disabled={refreshing}
                    className="gap-2"
                >
                    <ArrowPathIcon
                        className={classNames(
                            'h-4 w-4',
                            refreshing ? 'animate-spin' : ''
                        )}
                    />
                    {refreshing ? 'Refreshing...' : 'Refresh'}
                </Button>
            </div>

            {error && (
                <Card>
                    <CardContent className="py-4 text-sm text-destructive">
                        {error}
                    </CardContent>
                </Card>
            )}

            {services.length === 0 && !refreshing && !error && (
                <Card>
                    <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                        <div className="text-muted-foreground">
                            <h3 className="text-lg font-medium mb-2">No PACS services configured</h3>
                            <p className="text-sm">Configure PACS services in Aurabox, then click Refresh.</p>
                        </div>
                    </CardContent>
                </Card>
            )}

            {services.length > 0 && (
                <div className="space-y-3">
                    {services.map((service) => {
                        const echoState = echoes[service.id]
                        return (
                            <Card key={service.id}>
                                <CardContent className="p-4 flex items-center justify-between gap-4">
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2 mb-1">
                                            <span className="font-medium truncate">{service.label}</span>
                                            <Badge variant="secondary">{service.ae_title}</Badge>
                                        </div>
                                        <p className="text-sm text-muted-foreground">
                                            {service.host}:{service.port}
                                        </p>
                                    </div>

                                    <EchoStatus state={echoState} />

                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={() => echo(service)}
                                        disabled={echoState?.state === 'running'}
                                    >
                                        {echoState?.state === 'running' ? 'Echoing...' : 'Echo'}
                                    </Button>
                                </CardContent>
                            </Card>
                        )
                    })}
                </div>
            )}
        </div>
    )
}

function EchoStatus({ state }: { state?: EchoState }) {
    if (!state || state.state === 'idle' || state.state === 'running') {
        return null
    }

    if (state.state === 'ok') {
        return (
            <div className="flex items-center gap-1 text-sm text-green-600">
                <CheckCircleIcon className="h-4 w-4" />
                <span>{state.latencyMs ?? '?'} ms</span>
            </div>
        )
    }

    return (
        <div
            className="flex items-center gap-1 text-sm text-destructive max-w-xs truncate"
            title={state.error}
        >
            <XCircleIcon className="h-4 w-4 shrink-0" />
            <span className="truncate">{state.error}</span>
        </div>
    )
}
