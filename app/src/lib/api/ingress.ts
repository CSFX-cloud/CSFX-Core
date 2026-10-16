import { authedFetch } from './http';

const API_BASE = '/api';

export interface IngressRoute {
    id: string;
    resource_group_id: string;
    workload_id: string;
    hostname: string;
    container_port: number;
    tls: boolean;
    created_at: string;
}

export interface CreateIngressRequest {
    hostname: string;
    workload_id: string;
    container_port: number;
    tls: boolean;
}

async function failure(res: Response, fallback: string): Promise<Error> {
    const body = await res.json().catch(() => ({ error: `${fallback}: ${res.status}` }));
    return new Error(body.error ?? `${fallback}: ${res.status}`);
}

export async function listIngress(token: string, groupId: string): Promise<IngressRoute[]> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/ingress`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to list ingress routes');
    return res.json();
}

export async function createIngress(token: string, groupId: string, request: CreateIngressRequest): Promise<IngressRoute> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/ingress`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
    });
    if (!res.ok) throw await failure(res, 'Failed to create ingress route');
    return res.json();
}

export async function deleteIngress(token: string, groupId: string, routeId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/ingress/${routeId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to delete ingress route');
}
