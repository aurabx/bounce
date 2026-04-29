'use client';

import { invoke } from '@tauri-apps/api/core';
import {receiverStart, receiverStop} from "@/app/lib/server";
import {useAppSelector} from "@/app/lib/hook";
import { cn } from "@/app/lib/utils";
import {useSetupComplete} from "@/app/lib/customHooks";
import Settings from "@/app/components/Settings";
import ConnectivityStatus from "@/app/components/ConnectivityStatus";
import { Button } from "@/app/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui/card";
import { Badge } from "@/app/components/ui/badge";
import { Alert, AlertDescription } from "@/app/components/ui/alert";

export default function Page() {
    const running = useAppSelector((state) => state.main.running)
    const runningDetail = useAppSelector((state) => state.main.runningDetail)
    const studies = useAppSelector((state) => state.main.studies)
    const errors = useAppSelector((state) => state.main.errors)
    const {setupComplete} =  useSetupComplete();

    const sendLog = async () => {
        try {
            await invoke('send_log', { log: "Log and things" }); // Pass the port to Tauri
        } catch (error) {
            console.error('Error starting server:', error);
            alert(`Failed to start server: ${error}`);
        }
    };

    const loadStudies = async () => {
        await invoke('current_studies');
    }

    return (setupComplete ?
        <main className="space-y-8">
            <div className="flex flex-col items-center justify-center space-y-6 py-8">
                <h1 className="text-4xl font-bold tracking-tight">
                    Aurabox Bounce
                </h1>

                <Button
                    size="lg"
                    onClick={running ? receiverStop : receiverStart}
                    variant={running ? "destructive" : "default"}
                    className="h-16 px-8 text-lg"
                >
                    {running ? 'Stop Service' : 'Start Service'}
                </Button>
            </div>

            {errors.length > 0 && (
                <Alert variant="destructive">
                    <AlertDescription>
                        <ul className="list-disc pl-4 space-y-1">
                            {errors.map((error, index) => (
                                <li key={`error-${index}`}>{error}</li>
                            ))}
                        </ul>
                    </AlertDescription>
                </Alert>
            )}

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <Card>
                    <CardHeader className="pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Status</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="flex items-center justify-between">
                            <div className="text-2xl font-bold text-primary">
                                {running ? 'Running' : 'Stopped'}
                            </div>
                            <div className="flex h-4 w-4 relative">
                                {running && <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>}
                                <span className={cn(
                                    running ? 'bg-emerald-500' : 'bg-red-500',
                                    "relative inline-flex rounded-full h-4 w-4"
                                )}></span>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Current studies</CardTitle>
                    </CardHeader>
                    <CardContent>
                         <div className="text-2xl font-bold text-primary">
                            {studies.length}
                        </div>
                    </CardContent>
                </Card>

                <div className="md:col-span-2">
                    <ConnectivityStatus />
                </div>
            </div>

            {running && (
                <Card>
                    <CardHeader>
                         <CardTitle>Server Details</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                            {runningDetail.map((detail, key) => (
                                <div key={key} className="space-y-1">
                                    <p className="text-sm font-medium text-muted-foreground">{detail.label}</p>
                                    <p className="text-sm font-mono bg-muted p-2 rounded break-all">{detail.value}</p>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}
        </main> : (
                <div className="max-w-2xl mx-auto py-12 space-y-8">
                    <div className="text-center space-y-2">
                        <h1 className="text-3xl font-bold tracking-tight">
                            Welcome to Aurabox Bounce
                        </h1>
                        <p className="text-muted-foreground">
                            Please configure your API key, port and local storage location below.
                        </p>
                    </div>
                    <Settings/>
                </div>
            )
    );
}
