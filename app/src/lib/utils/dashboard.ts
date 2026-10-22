import type { AlertEvent } from "$lib/api/events";
import type { Bucket, Volume, Workload } from "$lib/api/resource-groups";
import type { UsageHistoryPoint } from "$lib/api/nodes";

export type Tone = "success" | "warning" | "danger";

export interface ServiceCounts {
    vms: number;
    containers: number;
    volumes: number;
    buckets: number;
    total: number;
}

export interface SeriesPaths {
    line: string;
    area: string;
}

const CHART_WIDTH = 600;
const CHART_HEIGHT = 200;
const CHART_BASELINE = CHART_HEIGHT - 1;
const MAX_PERCENT = 100;
const WARNING_USAGE = 70;
const DANGER_USAGE = 90;
const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;
const TIMEZONE_SUFFIX = /(Z|[+-]\d{2}:?\d{2})$/;

export function countServices(workloads: Workload[], volumes: Volume[], buckets: Bucket[]): ServiceCounts {
    const vms = workloads.filter((workload) => workload.runtime_class === "vm").length;
    const containers = workloads.length - vms;
    return { vms, containers, volumes: volumes.length, buckets: buckets.length, total: workloads.length + volumes.length + buckets.length };
}

export function usageTone(percent: number): "" | "warning" | "danger" {
    if (percent >= DANGER_USAGE) return "danger";
    if (percent >= WARNING_USAGE) return "warning";
    return "";
}

export function nodeTone(status: string): Tone {
    const normalized = status.toLowerCase();
    if (normalized === "online") return "success";
    return normalized === "offline" ? "danger" : "warning";
}

export function seriesPaths(samples: UsageHistoryPoint[], key: "cpu" | "memory" | "disk"): SeriesPaths {
    if (samples.length < 2) return { line: "", area: "" };
    const step = CHART_WIDTH / (samples.length - 1);
    const points = samples.map((sample, index) => {
        const x = index * step;
        const y = CHART_BASELINE - ((CHART_BASELINE - 1) * Math.min(sample[key], MAX_PERCENT)) / MAX_PERCENT;
        return `${x.toFixed(1)} ${y.toFixed(1)}`;
    });
    const line = `M${points.join("L")}`;
    const firstX = points[0].split(" ")[0];
    const lastX = points[points.length - 1].split(" ")[0];
    return { line, area: `${line}L${lastX} ${CHART_BASELINE}L${firstX} ${CHART_BASELINE}Z` };
}

export function pointPosition(index: number, count: number, percent: number): { left: number; top: number } {
    const left = count > 1 ? (index / (count - 1)) * 100 : 0;
    const y = CHART_BASELINE - ((CHART_BASELINE - 1) * Math.min(percent, MAX_PERCENT)) / MAX_PERCENT;
    return { left, top: (y / CHART_HEIGHT) * 100 };
}

export function formatBucket(bucket: string, range: "1h" | "24h" | "7d"): string {
    const date = new Date(parseTimestamp(bucket));
    const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    if (range !== "7d") return time;
    return `${date.toLocaleDateString([], { weekday: "short", day: "numeric" })} ${time}`;
}

export function alertTone(event: AlertEvent): Tone {
    if (event.status === "resolved") return "success";
    return event.severity === "critical" ? "danger" : "warning";
}

export function alertHref(event: AlertEvent): string {
    return event.agent_id ? `/nodes/${event.agent_id}?tab=alerts` : "/monitoring";
}

export function alertTitle(event: AlertEvent): string {
    const words = event.event_type.replaceAll("_", " ");
    return words.charAt(0).toUpperCase() + words.slice(1);
}

export function parseTimestamp(value: string): number {
    return new Date(TIMEZONE_SUFFIX.test(value) ? value : `${value}Z`).getTime();
}

export function relativeTime(value: string, now: number): string {
    const elapsed = Math.max(0, now - parseTimestamp(value));
    if (elapsed < MINUTE_MS) return "just now";
    if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} min ago`;
    if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)} h ago`;
    return `${Math.floor(elapsed / DAY_MS)} d ago`;
}
