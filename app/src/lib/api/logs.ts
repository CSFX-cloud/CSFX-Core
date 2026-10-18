import { authedFetch } from './http';

const API_BASE = '/api';

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
    target_id?: string;
    from?: string;
    to?: string;
    q?: string;
    limit?: number;
    offset?: number;
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

export async function listLogs(token: string, filter: LogsFilter): Promise<LogsResponse> {
    const res = await authedFetch(`${API_BASE}/logs${buildQuery(filter)}`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`Failed to list logs: ${res.status}`);
    return res.json();
}

export async function getLogsRetention(token: string): Promise<LogsRetention> {
    const res = await authedFetch(`${API_BASE}/admin/settings/logs-retention`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`Failed to get logs retention: ${res.status}`);
    return res.json();
}

export async function updateLogsRetention(
    token: string,
    retentionDays: number,
): Promise<LogsRetention> {
    const res = await authedFetch(`${API_BASE}/admin/settings/logs-retention`, {
        method: 'PUT',
        headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
        },
        body: JSON.stringify({ retention_days: retentionDays }),
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.status }));
        throw new Error(err.error ?? `Failed to update logs retention: ${res.status}`);
    }
    return res.json();
}
