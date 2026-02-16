'use client'

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAppDispatch, useAppSelector } from '@/app/lib/hook'
import { setDicomServices } from '@/app/lib/store'
import { DicomService } from '@/app/lib/types'
import { Card, CardContent, CardHeader, CardTitle } from '@/app/components/ui/card'
import { Badge } from '@/app/components/ui/badge'
import { Button } from '@/app/components/ui/button'

export default function PacsServices() {
    const dispatch = useAppDispatch()
    const services = useAppSelector((state) => state.main.dicomServices)
    const running = useAppSelector((state) => state.main.running)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    const fetchServices = async () => {
        setLoading(true)
        setError(null)
        try {
            const result = await invoke<DicomService[]>('fetch_dicom_services')
            dispatch(setDicomServices(result))
        } catch (err) {
            setError(String(err))
            dispatch(setDicomServices([]))
        } finally {
            setLoading(false)
        }
    }

    useEffect(() => {
        if (running) {
            fetchServices()
        }
    }, [running])

    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle>PACS Services</CardTitle>
                <Button
                    variant="outline"
                    size="sm"
                    onClick={fetchServices}
                    disabled={loading || !running}
                >
                    {loading ? 'Loading...' : 'Refresh'}
                </Button>
            </CardHeader>
            <CardContent>
                {!running && (
                    <p className="text-sm text-muted-foreground">
                        Start the receiver to fetch configured PACS services.
                    </p>
                )}

                {error && (
                    <p className="text-sm text-destructive">{error}</p>
                )}

                {running && !loading && services.length === 0 && !error && (
                    <p className="text-sm text-muted-foreground">
                        No PACS services configured in Aurabox.
                    </p>
                )}

                {services.length > 0 && (
                    <div className="space-y-3">
                        {services.map((service) => (
                            <div
                                key={service.id}
                                className="flex items-center justify-between rounded-lg border p-3"
                            >
                                <div className="space-y-1">
                                    <div className="flex items-center gap-2">
                                        <span className="font-medium">{service.label}</span>
                                        <Badge variant="secondary">{service.ae_title}</Badge>
                                    </div>
                                    <p className="text-sm text-muted-foreground">
                                        {service.host}:{service.port}
                                    </p>
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </CardContent>
        </Card>
    )
}
