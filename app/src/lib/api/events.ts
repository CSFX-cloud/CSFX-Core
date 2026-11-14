import { api } from './http';

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

export function listEvents(filters?: { status?: string; agentId?: string }): Promise<AlertEvent[]> {
    const params = new URLSearchParams();
    if (filters?.status) params.set('status', filters.status);
    if (filters?.agentId) params.set('agent_id', filters.agentId);
    const query = params.toString();
    return api<AlertEvent[]>('GET', `/events${query ? `?${query}` : ''}`);
}

export const resolveEvent = (eventId: string) =>
    api<void>('POST', `/events/${eventId}/resolve`);

export const setMaintenance = (agentId: string, minutes: number) =>
    api<void>('POST', `/agents/${agentId}/maintenance`, { minutes });

export const clearMaintenance = (agentId: string) =>
    api<void>('DELETE', `/agents/${agentId}/maintenance`);
