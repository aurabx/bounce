
export type NavigationItem = {
    label: string;
    href: string;
    title: string | false;
};

export const items: NavigationItem[] = [{
    label: "Dashboard",
    href: "/",
    title: false
},{
    label: "Studies",
    href: "/studies",
    title: "Studies"
},{
    label: "Logs",
    href: "/logs",
    title: "Logs"
},{
    label: "Settings",
    href: "/settings",
    title: "Settings"
},{
    label: "Tools",
    href: "/tools",
    title: "Tools"
}];


export const resolveTitleFromPath = (path: string) => {
    return items.find(item => item.href === path)?.title;
}


