import type { LogEntry } from '$lib/api/logs';

export const LEVELS = ['ERROR', 'WARN', 'INFO', 'DEBUG'] as const;

export const RANGES = [
    { key: '1H', label: '1H', ms: 3_600_000 },
    { key: '1D', label: '1D', ms: 86_400_000 },
    { key: '1W', label: '1W', ms: 604_800_000 },
    { key: '1M', label: '1M', ms: 2_592_000_000 },
] as const;

export type RangeKey = (typeof RANGES)[number]['key'] | 'custom';

export function rangeStart(range: RangeKey): string | undefined {
    const entry = RANGES.find((candidate) => candidate.key === range);
    return entry ? new Date(Date.now() - entry.ms).toISOString() : undefined;
}

export function csv(values: string[]): string | undefined {
    return values.length > 0 ? values.join(',') : undefined;
}

export function toggleValue(values: string[], value: string): string[] {
    return values.includes(value) ? values.filter((entry) => entry !== value) : [...values, value];
}

export function levelBars(level: string): number {
    switch (level) {
        case 'ERROR': return 3;
        case 'WARN': return 2;
        default: return 1;
    }
}

export function levelBarClass(level: string): string {
    switch (level) {
        case 'ERROR': return 'bg-red-500';
        case 'WARN': return 'bg-yellow-500';
        case 'DEBUG': return 'bg-muted-foreground';
        default: return 'bg-green-500';
    }
}

export function levelTextClass(level: string): string {
    switch (level) {
        case 'ERROR': return 'text-red-500';
        case 'WARN': return 'text-yellow-500';
        case 'DEBUG': return 'text-muted-foreground';
        default: return 'text-green-500';
    }
}

export function rawLogLine(entry: LogEntry): string {
    const fields = Object.entries(entry.fields ?? {})
        .map(([key, value]) => `${key}=${String(value)}`)
        .join(' ');
    const head = `[${entry.level}] ${entry.service} ${entry.message}`;
    return fields ? `${head} ${fields}` : head;
}
