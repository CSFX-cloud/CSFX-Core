import { authedFetch } from './http';

const API_BASE = '/api';

export interface AlertEvent {
    id: string;
    agent_id: string | null;
    event_type: string;
    affected_workloads: string[] | null;
    duration_ms: number | null;
    created_at: string;
    severity: string;
    status: string;
    resolved_at: string | null;
    message: string | null;
}

export async function listEvents(
    token: string,
    filters?: { status?: string; agentId?: string },
): Promise<AlertEvent[]> {
    const params = new URLSearchParams();
    if (filters?.status) params.set('status', filters.status);
    if (filters?.agentId) params.set('agent_id', filters.agentId);
    const query = params.toString();
    const res = await authedFetch(`${API_BASE}/events${query ? `?${query}` : ''}`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`events fetch failed: ${res.status}`);
    return res.json();
}

export async function setMaintenance(token: string, agentId: string, minutes: number): Promise<void> {
    const res = await authedFetch(`${API_BASE}/agents/${agentId}/maintenance`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ minutes }),
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.status }));
        throw new Error(err.error ?? `Failed to set maintenance: ${res.status}`);
    }
}

export async function clearMaintenance(token: string, agentId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/agents/${agentId}/maintenance`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.status }));
        throw new Error(err.error ?? `Failed to clear maintenance: ${res.status}`);
    }
}
