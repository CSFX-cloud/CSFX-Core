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

export interface LevelMeta {
    label: string;
    tone: 'danger' | 'warning' | '';
    dot: string;
}

const LEVEL_META: Record<string, LevelMeta> = {
    ERROR: { label: 'Error', tone: 'danger', dot: 'var(--danger)' },
    WARN: { label: 'Warning', tone: 'warning', dot: 'var(--warning)' },
    INFO: { label: 'Info', tone: '', dot: 'var(--ink-soft)' },
    DEBUG: { label: 'Debug', tone: '', dot: 'var(--marker)' },
};

export function levelMeta(level: string): LevelMeta {
    return LEVEL_META[level] ?? LEVEL_META.INFO;
}

const PAST_TENSE: Record<string, string> = {
    create: 'created',
    delete: 'deleted',
    update: 'updated',
    stop: 'stopped',
    start: 'started',
    restart: 'restarted',
    redeploy: 'redeployed',
    login: 'signed in',
    login_failed: 'failed to sign in',
    ticket: 'requested a console ticket for',
};

function singular(resource: string): string {
    return resource.replace(/-/g, ' ').replace(/s$/, '');
}

export function describeAction(entry: LogEntry): string {
    if (!entry.action) return entry.message;
    const [resource, verb] = entry.action.split('.');
    const verbText = PAST_TENSE[verb] ?? verb;
    return entry.action.startsWith('auth.') ? verbText : `${verbText} ${singular(resource)}`;
}
