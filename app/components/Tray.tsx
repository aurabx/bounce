"use client";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { defaultWindowIcon } from "@tauri-apps/api/app";
import { TrayIcon, type TrayIconOptions } from "@tauri-apps/api/tray";
import { exit, relaunch } from "@tauri-apps/plugin-process";
// import { open } from "@tauri-apps/plugin-shell";
import {useEffect, useState} from "react";
import {resolveResource} from "@tauri-apps/api/path";
import {useAppSelector} from "@/app/lib/hook";
import {receiverStart, receiverStop} from "@/app/lib/server";

const TRAY_ID = 'bounce';

const appName = 'Bounce';
const appVersion = '1.0.1';

const Tray = () => {
    const running = useAppSelector((state) => state.main.running)

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

        if (tray) return;

        const menu = await getTrayMenu();

        const iconPath = "icons/icon.ico";
        // const icon = await defaultWindowIcon() ?? undefined;
        const icon = await resolveResource(iconPath) ?? undefined;

        const options: TrayIconOptions = {
            menu,
            icon,
            id: TRAY_ID,
            tooltip: `${appName} v${appVersion}`,
            iconAsTemplate: true,
            menuOnLeftClick: true,
        };

        return TrayIcon.new(options);
    };

    const getTrayMenu = async () => {

        const items = await Promise.all([
            MenuItem.new({
                text: running
                    ? 'Stop'
                    : 'Start',
                action: () => running ? receiverStop() : receiverStart(),
            }),
            PredefinedMenuItem.new({ item: "Separator" }),
            MenuItem.new({
                text: `Version ${appVersion}`,
                enabled: false,
            }),
            MenuItem.new({
                text: `Relaunch`,
                action: relaunch,
            }),
            MenuItem.new({
                text: `Exit`,
                action: () => exit(0),
            }),
        ]);

        return Menu.new({ items });
    };



    return <></>;
};

export default Tray;