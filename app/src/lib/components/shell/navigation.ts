import { Activity, Database, Layers, LayoutDashboard, ScrollText, Server, Settings } from "@lucide/svelte";

export interface NavItem {
    title: string;
    href: string;
    icon: typeof Server;
}

export const PRIMARY_NAV: NavItem[] = [
    { title: "Overview", href: "/", icon: LayoutDashboard },
    { title: "Nodes", href: "/nodes", icon: Server },
    { title: "Resource groups", href: "/resource-groups", icon: Layers },
    { title: "Monitoring", href: "/monitoring", icon: Activity },
    { title: "Logs", href: "/logs", icon: ScrollText },
];

export const RESOURCE_NAV: NavItem[] = [{ title: "S3 buckets", href: "/buckets", icon: Database }];

export const SETTINGS_NAV: NavItem = { title: "Settings", href: "/admin/settings", icon: Settings };

const SECTIONS = [...PRIMARY_NAV, ...RESOURCE_NAV, SETTINGS_NAV];

export function isActive(href: string, pathname: string): boolean {
    if (href === "/") return pathname === "/";
    return pathname === href || pathname.startsWith(`${href}/`);
}

export function findSection(pathname: string): NavItem {
    return SECTIONS.find((item) => isActive(item.href, pathname)) ?? PRIMARY_NAV[0];
}
