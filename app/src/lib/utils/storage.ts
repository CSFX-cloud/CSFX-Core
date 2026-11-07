import type { UsageEntry, Volume, Workload } from "$lib/api/resource-groups";

const BYTES_PER_GB = 1_073_741_824;
const PERCENT = 100;

export interface StorageRow {
    volume: Volume;
    mountPath: string;
    usedBytes: number | null;
    totalBytes: number;
    entries: UsageEntry[];
}

export function storageRows(workloads: Workload[], volumes: Volume[]): StorageRow[] {
    return workloads.flatMap((workload) =>
        (workload.volume_mounts ?? []).flatMap((mount) => {
            const volume = volumes.find((candidate) => candidate.id === mount.volume_id);
            if (!volume) return [];
            const usage = workload.volume_usage?.find((entry) => entry.mount_path === mount.mount_path);
            return [{ volume, mountPath: mount.mount_path, usedBytes: usage?.used_bytes ?? null, totalBytes: usage?.total_bytes ?? volume.size_gb * BYTES_PER_GB, entries: usage?.entries ?? [] }];
        }),
    );
}

const SEGMENT_COLORS = ["var(--chart-memory)", "var(--chart-cpu)", "var(--warning)", "var(--danger)", "var(--success)"];
const OTHER_COLOR = "var(--ink-muted)";

export interface UsageSegment {
    name: string;
    bytes: number;
    color: string;
}

export function usageSegments(row: StorageRow): UsageSegment[] {
    if (row.usedBytes === null) return [];
    const listed = row.entries.map((entry, index): UsageSegment => ({ name: entry.name, bytes: entry.bytes, color: SEGMENT_COLORS[index % SEGMENT_COLORS.length] }));
    const other = row.usedBytes - listed.reduce((sum, segment) => sum + segment.bytes, 0);
    return other > 0 ? [...listed, { name: "Other", bytes: other, color: OTHER_COLOR }] : listed;
}

export function usedPercent(usedBytes: number | null, totalBytes: number): number | null {
    if (usedBytes === null || totalBytes <= 0) return null;
    return Math.min(PERCENT, (usedBytes / totalBytes) * PERCENT);
}
