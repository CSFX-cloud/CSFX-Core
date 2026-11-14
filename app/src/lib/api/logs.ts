import { api, request } from './http';
import { saveBlob } from '$lib/utils/download';

export interface LogEntry {
    id: string;
    service: string;
    level: string;
    classification: string;
    message: string;
    agent_id: string | null;
    workload_id: string | null;
    organization_id: string | null;
    created_at: string;
    kind: 'user' | 'system';
    actor_user_id: string | null;
    action: string | null;
    target_type: string | null;
    target_id: string | null;
    fields: Record<string, unknown>;
    occurrences: number;
    last_seen_at: string;
    cef: string;
}

export interface LogsResponse {
    entries: LogEntry[];
    total: number;
    has_more: boolean;
}

export interface LogsFilter {
    service?: string;
    level?: string;
    classification?: string;
    agent_id?: string;
    workload_id?: string;
    organization_id?: string;
    kind?: 'user' | 'system';
    actor_user_id?: string;
    action?: string;
    target_type?: string;
    target_id?: string;
    from?: string;
    to?: string;
    q?: string;
    limit?: number;
    offset?: number;
}

export interface FacetCount {
    value: string;
    count: number;
}

export interface LogsFacets {
    kinds: FacetCount[];
    classifications: FacetCount[];
    services: FacetCount[];
    target_types: FacetCount[];
    actions: FacetCount[];
}

export interface LogsRetention {
    retention_days: number;
}

function buildQuery(filter: LogsFilter): string {
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(filter)) {
        if (value !== undefined && value !== null && value !== '') {
            params.set(key, String(value));
        }
    }
    const query = params.toString();
    return query ? `?${query}` : '';
}

export const listLogs = (filter: LogsFilter) =>
    api<LogsResponse>('GET', `/logs${buildQuery(filter)}`);

export interface LogsTrendPoint {
    start: string;
    count: number;
}

export const getLogsTrend = (filter: LogsFilter) =>
    api<LogsTrendPoint[]>('GET', `/logs/trend${buildQuery(filter)}`);

export const getLogsFacets = (filter: LogsFilter) =>
    api<LogsFacets>('GET', `/logs/facets${buildQuery(filter)}`);

export async function exportLogsCef(filter: LogsFilter): Promise<void> {
    const res = await request('GET', `/logs/export${buildQuery(filter)}`);
    saveBlob(await res.blob(), 'csfx-logs.cef');
}

export const getLogsRetention = () =>
    api<LogsRetention>('GET', '/admin/settings/logs-retention');

export const updateLogsRetention = (retentionDays: number) =>
    api<LogsRetention>('PUT', '/admin/settings/logs-retention', { retention_days: retentionDays });
