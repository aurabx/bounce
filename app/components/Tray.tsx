"use client";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { getVersion, getName } from "@tauri-apps/api/app";
import { TrayIcon, type TrayIconOptions } from "@tauri-apps/api/tray";
import {useEffect, useRef} from "react";
import {resolveResource} from "@tauri-apps/api/path";
import {useAppSelector} from "@/app/lib/hook";
import {receiverStart, receiverStop} from "@/app/lib/server";
import {invokeCommand} from "@/app/lib/commands";
import {persistRunningStateAndRelaunch} from "@/app/lib/relaunch";

const TRAY_ID = 'bounce';

const Tray = () => {
    const running = useAppSelector((state) => state.main.running)
    // Mirror running into a ref so the tray menu's Relaunch action always
    // reads the current value, even if it runs against a stale menu closure
    // (the menu is rebuilt async when `running` changes).
    const runningRef = useRef(running)
    useEffect(() => {
        runningRef.current = running
    }, [running])


    useEffect(() => {
        createTrayIcon().finally();
    }, []);

    const updateTrayMenu = async () => {
        const tray = await getTrayById();

        if (!tray) return;

        const menu = await getTrayMenu();

        await tray.setMenu(menu);
    };


    useEffect(() => {
        updateTrayMenu();
    }, [running]);


    const getTrayById = () => {
        return TrayIcon.getById(TRAY_ID);
    };

    const createTrayIcon = async () => {

        const tray = await getTrayById();
        const version = await getVersion();
        const name = await getName();

        if (tray) return;

        const menu = await getTrayMenu();

        const iconPath = "icons/icon.ico";
        // const icon = await defaultWindowIcon() ?? undefined;
        const icon = await resolveResource(iconPath) ?? undefined;

        const options: TrayIconOptions = {
            menu,
            icon,
            id: TRAY_ID,
            tooltip: `${name} v${version}`,
            iconAsTemplate: true,
            menuOnLeftClick: true,
        };

        return TrayIcon.new(options);
    };


    const show = async () => {
        await invokeCommand('show_window');
    }

    const getTrayMenu = async () => {
        const version = await getVersion();
        const name = await getName();

        const items = await Promise.all([
            MenuItem.new({
                text: running
                    ? 'Stop'
                    : 'Start',
                action: () => running ? receiverStop() : receiverStart(),
            }),
            MenuItem.new({
                text: `Show window`,
                action: show,
            }),
            PredefinedMenuItem.new({ item: "Separator" }),
            MenuItem.new({
                text: `${name} v${version}`,
                enabled: false,
            }),
            MenuItem.new({
                text: `Relaunch`,
                action: () => persistRunningStateAndRelaunch(runningRef.current),
            }),
            MenuItem.new({
                text: `Exit`,
                action: () => invokeCommand('request_exit'),
            }),
        ]);

        return Menu.new({ items });
    };



    return <></>;
};

export default Tray;