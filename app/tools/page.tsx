'use client';

import {FormEvent, Suspense, useEffect, useState} from 'react'
import {invoke} from "@tauri-apps/api/core";
import TextInput from "@/app/components/Fields/TextInput";

export default function Page() {

    const [study_uid, setStudyUid] = useState<string>('1.3.46.670589.11.3540642177.2867929537.1763690001.2563942908');
    const [signature, setSignature] = useState<string>('8a5b26c5485049200a1167df5efc664ee9c6f115');
    const [upload_id, setUploadId] = useState<string>('b4a9764d-75e6-4ac0-8235-9f8189289353');

    const testStartUpload = async (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault()

        try {
            await invoke('start_upload', {
                studyUid: study_uid,
                signature: signature,
                uploadId: upload_id,
            });
        } catch (error) {
            console.error('Error start_upload:', error);
            alert(`Failed to start_upload: ${error}`);
        }
    };


    return (
        <>
            <h1 className="text-3xl font-bold mb-6">
                Tools
            </h1>
            <div className="bg-white shadow-lg rounded-lg p-6 w-full">
                <Suspense fallback={<Loading />}>
                    <h3 className="mb-3 font-bold text-lg">Send study to aura</h3>
                    <form onSubmit={testStartUpload}>
                        <div className="space-y-6">

                            <TextInput
                                config={({label: 'Study UID', key: 'study_uid'})}
                                value={study_uid}
                                onChange={(e: any) => setStudyUid(e.target.value)}
                            />

                            <TextInput
                                config={({label: 'Signature', key: 'signature'})}
                                value={signature}
                                onChange={(e: any) => setSignature(e.target.value)}
                            />

                            <TextInput
                                config={({label: 'Upload ID', key: 'upload_id'})}
                                value={upload_id}
                                onChange={(e: any) => setUploadId(e.target.value)}
                            />

                            <div className="mb-4 flex justify-end">
                                <button
                                    type="submit"
                                    className="inline-flex grow-0 transition ease-in-out text-center border shadow-sm font-medium rounded-md px-4 py-2 text-sm cursor-pointer text-white bg-indigo-400 hover:bg-indigo-500"
                                >
                                    Send
                                </button>
                            </div>
                        </div>
                    </form>
                </Suspense>
            </div>
        </>
    );
}

function Loading() {
    return <h2>🌀 Loading...</h2>;
}