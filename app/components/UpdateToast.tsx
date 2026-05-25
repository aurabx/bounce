'use client';

import { ArrowPathIcon } from '@heroicons/react/24/outline';
import { useUpdate } from '@/app/lib/UpdateContext';
import { Button } from '@/app/components/ui/button';

export default function UpdateToast() {
    const { status, updateInfo, restartApp } = useUpdate();

    if (status !== 'ready') return null;

    return (
        <div className="flex items-center gap-3 px-4 py-2.5 bg-primary/10 border-b border-primary/30 text-sm">
            <ArrowPathIcon className="h-4 w-4 text-primary flex-shrink-0" />
            <span className="flex-1">
                {updateInfo
                    ? `Bounce ${updateInfo.version} is ready to install.`
                    : 'An update is ready to install.'}
            </span>
            <Button size="sm" onClick={restartApp}>
                Restart Now
            </Button>
        </div>
    );
}
