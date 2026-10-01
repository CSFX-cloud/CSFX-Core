import type { ClusterStats, HealthHistoryPoint } from "$lib/api/nodes";

export interface UsageSample {
    cpu: number;
    memory: number;
    disk: number;
}

const HEALTHY_UPTIME = 99.9;
const DEGRADED_UPTIME = 99;
const BUCKET_SECONDS = 3600;
const MS_PER_SECOND = 1000;

function percent(used: number, total: number): number {
    return total > 0 ? Math.min(100, (used / total) * 100) : 0;
}

export function uptimeColor(uptimePercent: number | null): string {
    if (uptimePercent === null) return "currentColor";
    if (uptimePercent >= HEALTHY_UPTIME) return "#22c55e";
    if (uptimePercent >= DEGRADED_UPTIME) return "#eab308";
    return "#ef4444";
}

export function toUsageSample(stats: ClusterStats): UsageSample {
    return {
        cpu: Math.min(100, stats.avg_cpu_usage_percent),
        memory: percent(stats.used_memory_bytes, stats.total_memory_bytes),
        disk: percent(stats.used_disk_bytes, stats.total_disk_bytes),
    };
}

export function clusterUptimePercent(points: HealthHistoryPoint[], nodeCount: number): number | null {
    if (points.length === 0 || nodeCount === 0) return null;
    const first = new Date(`${points[0].bucket}Z`).getTime();
    const last = new Date(`${points[points.length - 1].bucket}Z`).getTime();
    const expectedBuckets = Math.round((last - first) / (BUCKET_SECONDS * MS_PER_SECOND)) + 1;
    const available = points.reduce((sum, point) => sum + Math.min(1, point.online_count / nodeCount), 0);
    return Math.min(100, (available / expectedBuckets) * 100);
}
