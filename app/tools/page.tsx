'use client';

import {FormEvent, Suspense, useEffect, useState} from 'react'
import {invoke} from "@tauri-apps/api/core";
import TextInput from "@/app/components/Fields/TextInput";
import SelectInput from "@/app/components/Fields/SelectInput";
import {useAppSelector} from "@/app/lib/hook";
import {Study} from "@/app/lib/types";
import { v4 as uuidv4 } from 'uuid';
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui/card";
import { Button } from "@/app/components/ui/button";
import PacsServices from "@/app/components/PacsServices";

type Commands = {
    [key: string]: string
}

export default function Page() {

    const studies = useAppSelector((state) => state.main.studies)

    const [loaded, setLoaded] = useState<boolean>(false);
    const [study_uid, setStudyUid] = useState<string>(studies.length > 0 ? studies[0].study_uid : '');
    const [signature, setSignature] = useState<string>('8a5b26c5485049200a1167df5efc664ee9c6f115');
    const [upload_id, setUploadId] = useState<string>('5bbfd5f0-3602-4ad7-8b1b-5ffa466fff8a');
    const [assembly_id, setAssemblyId] = useState<string>('4895b6a75bd44943adaeff6ec50d23a1');

    const [command, setCommand] = useState<string>('api_start_upload');
    const commands: Commands = {
        api_start_upload: 'Send api start request to aura',
        send_study: 'Send stored studies',
    };

    useEffect(() => {
        invoke('current_studies').then(() => {
            setLoaded(true)
        });
    }, []);

    const testStartUpload = async (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault()

        try {
            if (command === 'api_start_upload')
                await invoke('api_start_upload', {
                    studyUid: study_uid,
                    signature: signature,
                    uploadId: upload_id,
                    assemblyId: assembly_id,
                });
            else if (command === 'send_study'){
                await invoke('send_study', {
                    studyUid: study_uid,
                });
            }

        } catch (error) {
            console.error(`Error ${command}:`, error);
            alert(`Error ${command}: ${error}`);
        }
    };

    const generateUploadId = () => {
        setUploadId(uuidv4());
    }

    return (
        <>
            <PacsServices />

            <Card className="mt-4">
                <CardHeader>
                     <CardTitle>Developer Tools</CardTitle>
                </CardHeader>
                <CardContent>
                    <Suspense fallback={<Loading />}>
                    {(loaded ? <>
                        <SelectInput
                            config={({
                                label: 'Command',
                                key: 'command',
                                options: commands
                            })}
                            value={command}
                            onChange={(e: any) => setCommand(e.target.value)}
                        />

                        <h3 className="my-4 font-bold text-lg text-primary">{commands[command] || 'Unknown command'}</h3>

                        <form onSubmit={testStartUpload}>
                            <div className="space-y-6">
                                <SelectInput
                                    config={({
                                        label: 'Study UID',
                                        key: 'study_uid',
                                        options: studies.reduce((acc: {[key: string]: string}, study: Study) => {
                                            acc[study.study_uid] = study.study_uid;
                                            return acc;
                                        }, {})
                                    })}
                                    value={study_uid}
                                    onChange={(e: any) => setStudyUid(e.target.value)}
                                />

                                {command === 'api_start_upload' && <>
                                    <TextInput
                                        config={({label: 'Signature', key: 'signature'})}
                                        value={signature}
                                        onChange={(e: any) => setSignature(e.target.value)}
                                    />

                                    <TextInput
                                        config={({
                                            label: 'Upload ID',
                                            key: 'upload_id',
                                            suffix_button: 'Generate'
                                        })}
                                        value={upload_id}
                                        onSuffixClick={() => generateUploadId()}
                                        onChange={(e: any) => setUploadId(e.target.value)}
                                    />

                                    <TextInput
                                        config={({
                                            label: 'Assembly ID',
                                            key: 'assembly_id',
                                        })}
                                        value={assembly_id}
                                        onChange={(e: any) => setAssemblyId(e.target.value)}
                                    />
                                </>}
                                <div className="mb-4 flex justify-end">
                                    <Button
                                        type="submit"
                                    >
                                        Send
                                    </Button>
                                </div>
                            </div>
                        </form>
                    </> : null)}
                    </Suspense>
                </CardContent>
            </Card>
        </>
    );
}

function Loading() {
    return <h2>Loading...</h2>;
}
