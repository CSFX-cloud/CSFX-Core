import type { DiskInfo } from "$lib/api/nodes";
import type { HistoryPoint } from "$lib/components/nodes/detail/node-live.svelte";
import type { Tone } from "$lib/utils/dashboard";

export interface TrafficPaths {
    rx: string;
    tx: string;
}

const PERCENT = 100;
const BASE_BAY_COUNT = 8;
const BAYS_PER_ROW = 4;
const MIN_TRAFFIC_CEILING = 1024;
const TRAFFIC_WIDTH = 600;
const TRAFFIC_HEIGHT = 140;
const TRAFFIC_BASELINE = TRAFFIC_HEIGHT - 1;

export function clampPercent(value: number | null | undefined): number {
    if (value == null) return 0;
    return Math.min(Math.max(value, 0), PERCENT);
}

export function workloadTone(status: string): Tone {
    const normalized = status.toLowerCase();
    if (["running", "active", "available", "in_use"].includes(normalized)) return "success";
    return ["failed", "error", "offline"].includes(normalized) ? "danger" : "warning";
}

export function smartTone(health: string | undefined): Tone | "" {
    switch (health?.toLowerCase()) {
        case "healthy":
            return "success";
        case "failing":
            return "danger";
        default:
            return "";
    }
}

export function diskUsage(disk: DiskInfo): number {
    return disk.total_bytes > 0 ? clampPercent((disk.used_bytes / disk.total_bytes) * PERCENT) : 0;
}

export function isOptical(disk: DiskInfo): boolean {
    return /^sr\d|cdrom|dvd/i.test(`${disk.device} ${disk.media_type}`);
}

export function bayCount(diskCount: number): number {
    return Math.max(BASE_BAY_COUNT, Math.ceil(diskCount / BAYS_PER_ROW) * BAYS_PER_ROW);
}

export function bayLabel(index: number): string {
    return String(index).padStart(2, "0");
}

export function trafficCeiling(points: HistoryPoint[]): number {
    const peak = Math.max(0, ...points.map((point) => Math.max(point.rxBps, point.txBps)));
    if (peak <= MIN_TRAFFIC_CEILING) return MIN_TRAFFIC_CEILING;
    const magnitude = 10 ** Math.floor(Math.log10(peak));
    return Math.ceil(peak / magnitude) * magnitude;
}

function trafficLine(values: number[], ceiling: number): string {
    const step = TRAFFIC_WIDTH / (values.length - 1);
    return values
        .map((value, index) => {
            const y = TRAFFIC_BASELINE - ((TRAFFIC_BASELINE - 1) * Math.min(value, ceiling)) / ceiling;
            return `${index ? "L" : "M"}${(index * step).toFixed(1)} ${y.toFixed(1)}`;
        })
        .join("");
}

export function trafficPaths(points: HistoryPoint[], ceiling: number): TrafficPaths {
    if (points.length < 2) return { rx: "", tx: "" };
    return {
        rx: trafficLine(points.map((point) => point.rxBps), ceiling),
        tx: trafficLine(points.map((point) => point.txBps), ceiling),
    };
}

export function resolvedPercent(percent: number | null | undefined, used: number | null | undefined, total: number | null | undefined): number | null {
    if (percent != null) return percent;
    return used != null && total ? (used / total) * PERCENT : null;
}
