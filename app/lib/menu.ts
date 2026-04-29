import {
    HomeIcon,
    InboxStackIcon,
    CommandLineIcon,
    Cog6ToothIcon,
    WrenchScrewdriverIcon,
    ServerStackIcon
} from '@heroicons/react/24/outline';

export type NavigationItem = {
    label: string;
    href: string;
    title: string | false;
    icon: any;
};

export const items: NavigationItem[] = [{
    label: "Dashboard",
    href: "/",
    title: false,
    icon: HomeIcon
},{
    label: "Studies",
    href: "/studies",
    title: "Studies",
    icon: InboxStackIcon
},{
    label: "PACS",
    href: "/pacs",
    title: "PACS",
    icon: ServerStackIcon
},{
    label: "Logs",
    href: "/logs",
    title: "Logs",
    icon: CommandLineIcon
},{
    label: "Settings",
    href: "/settings",
    title: "Settings",
    icon: Cog6ToothIcon
},{
    label: "Tools",
    href: "/tools",
    title: "Tools",
    icon: WrenchScrewdriverIcon
}];


export const resolveTitleFromPath = (path: string) => {
    return items.find(item => item.href === path)?.title;
}


