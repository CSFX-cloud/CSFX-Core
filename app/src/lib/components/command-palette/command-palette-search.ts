import type { Component } from "svelte";

export interface PaletteItem {
    id: string;
    group: string;
    title: string;
    subtitle: string;
    kind: string;
    icon: Component;
    keywords: string;
    meta: [string, string][];
    cta: string;
    href: string;
    action?: () => void;
}

export type PaletteRow = { head: true; label: string } | { head: false; item: PaletteItem; index: number };

export interface SearchResult {
    rows: PaletteRow[];
    flat: PaletteItem[];
}

const GROUP_ORDER = ["Actions", "Pages", "Nodes", "Workloads", "Resource groups", "Buckets", "Alerts"];
const GROUP_LIMIT = 4;
const SUGGESTION_LIMIT = 3;

function score(item: PaletteItem, query: string): number {
    const title = item.title.toLowerCase();
    const position = title.indexOf(query);
    if (position === 0) return 0;
    if (position > 0) return 1;
    return `${item.subtitle} ${item.keywords} ${item.group}`.toLowerCase().includes(query) ? 2 : -1;
}

function rank(items: PaletteItem[], query: string): PaletteItem[] {
    return items
        .map((item, order) => ({ item, order, points: score(item, query) }))
        .filter((hit) => hit.points >= 0)
        .sort((a, b) => a.points - b.points || a.order - b.order)
        .map((hit) => hit.item);
}

export function search(items: PaletteItem[], rawQuery: string): SearchResult {
    const hits = rank(items, rawQuery.trim().toLowerCase());
    if (!hits.length) return { rows: [], flat: [] };
    const [top, ...rest] = hits;
    const flat = [top];
    const rows: PaletteRow[] = [
        { head: true, label: "Top hit" },
        { head: false, item: top, index: 0 },
    ];
    for (const group of GROUP_ORDER) {
        const members = rest.filter((item) => item.group === group).slice(0, GROUP_LIMIT);
        if (!members.length) continue;
        rows.push({ head: true, label: group });
        for (const item of members) {
            rows.push({ head: false, item, index: flat.length });
            flat.push(item);
        }
    }
    return { rows, flat };
}

export function suggestions(items: PaletteItem[]): PaletteItem[] {
    return items.filter((item) => item.group !== "Actions" && item.group !== "Pages").slice(0, SUGGESTION_LIMIT);
}

export function highlight(title: string, rawQuery: string): [string, string, string] {
    const query = rawQuery.trim().toLowerCase();
    const start = query ? title.toLowerCase().indexOf(query) : -1;
    if (start < 0) return [title, "", ""];
    return [title.slice(0, start), title.slice(start, start + query.length), title.slice(start + query.length)];
}
