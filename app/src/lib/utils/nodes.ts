import type { Node, NodeMetrics } from "$lib/api/nodes";

export type SortColumn = "hostname" | "status" | "cpu" | "memory" | "disk";
export type StatusTab = "all" | "online" | "offline";

const PERCENT = 100;
const MINUTE_MS = 60_000;
const MINUTES_PER_HOUR = 60;
const HEARTBEAT_LENGTH = 16;

export function ratio(used: number | null | undefined, total: number | null | undefined): number | null {
    if (used == null || !total) return null;
    return (used / total) * PERCENT;
}

export function memoryPercent(metrics: NodeMetrics | undefined): number | null {
    return ratio(metrics?.memory_used_bytes, metrics?.memory_total_bytes);
}

export function diskPercent(metrics: NodeMetrics | undefined): number | null {
    return ratio(metrics?.disk_used_bytes, metrics?.disk_total_bytes);
}

export function sortValue(node: Node, metrics: NodeMetrics | undefined, column: SortColumn): number | string {
    switch (column) {
        case "hostname":
            return node.hostname.toLowerCase();
        case "status":
            return node.status.toLowerCase();
        case "cpu":
            return metrics?.cpu_usage_percent ?? -1;
        case "memory":
            return memoryPercent(metrics) ?? -1;
        case "disk":
            return diskPercent(metrics) ?? -1;
    }
}

export function matchesTab(node: Node, tab: StatusTab): boolean {
    return tab === "all" || node.status.toLowerCase() === tab;
}

export function matchesQuery(node: Node, query: string): boolean {
    const needle = query.trim().toLowerCase();
    if (!needle) return true;
    return node.hostname.toLowerCase().includes(needle) || (node.ip_address ?? "").includes(needle);
}

export function isInMaintenance(node: Node): boolean {
    return node.maintenance_until !== null && new Date(node.maintenance_until).getTime() > Date.now();
}

export function maintenanceLabel(maintenanceUntil: string): string {
    const remaining = new Date(maintenanceUntil).getTime() - Date.now();
    if (remaining <= 0) return "";
    const totalMinutes = Math.ceil(remaining / MINUTE_MS);
    const hours = Math.floor(totalMinutes / MINUTES_PER_HOUR);
    const minutes = totalMinutes % MINUTES_PER_HOUR;
    return hours > 0 ? `${hours}h ${minutes}m left` : `${minutes}m left`;
}

export function formatHeartbeat(value: string | null): string {
    return value ? value.slice(0, HEARTBEAT_LENGTH).replace("T", " ") : "never";
}

export function nodesCsv(nodes: Node[], metricsById: Map<string, NodeMetrics>): string {
    const header = ["ID", "Hostname", "IP", "OS", "Architecture", "Status", "CPU %", "Memory %", "Disk %", "Heartbeat"];
    const rows = nodes.map((node) => {
        const metrics = metricsById.get(node.id);
        return [
            node.id,
            node.hostname,
            node.ip_address ?? "",
            `${node.os_type} ${node.os_version}`,
            node.architecture,
            node.status,
            metrics?.cpu_usage_percent?.toFixed(1) ?? "",
            memoryPercent(metrics)?.toFixed(1) ?? "",
            diskPercent(metrics)?.toFixed(1) ?? "",
            node.last_heartbeat ?? "",
        ];
    });
    return [header, ...rows].map((row) => row.map((cell) => `"${String(cell).replace(/"/g, '""')}"`).join(",")).join("\n");
}
