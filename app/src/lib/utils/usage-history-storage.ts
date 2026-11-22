import type { UsageHistoryPoint } from "$lib/api/nodes";

const KEY_PREFIX = "csfx.usage-history.";
const WINDOW_MS = 60 * 60 * 1000;

export const SAMPLE_CAPACITY = 720;

function isPoint(value: unknown): value is UsageHistoryPoint {
    const point = value as UsageHistoryPoint;
    return typeof point?.bucket === "string" && typeof point.cpu === "number" && typeof point.memory === "number";
}

export function loadUsageSamples(workloadId: string): UsageHistoryPoint[] {
    const raw = localStorage.getItem(KEY_PREFIX + workloadId);
    if (raw === null) return [];
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        localStorage.removeItem(KEY_PREFIX + workloadId);
        return [];
    }
    const cutoff = Date.now() - WINDOW_MS;
    return Array.isArray(parsed) ? parsed.filter((point) => isPoint(point) && Date.parse(point.bucket) >= cutoff) : [];
}

export function saveUsageSamples(workloadId: string, samples: UsageHistoryPoint[]) {
    localStorage.setItem(KEY_PREFIX + workloadId, JSON.stringify(samples));
}
