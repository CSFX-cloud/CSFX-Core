import { authedFetch } from './http';

const API_BASE = '/api';

export interface ClusterVip {
    address: string;
    prefix_length: number;
    router_id: number;
}

export async function getClusterVip(token: string): Promise<ClusterVip | null> {
    const res = await authedFetch(`${API_BASE}/cluster/vip`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`Failed to get cluster vip: ${res.status}`);
    const body: { vip: ClusterVip | null } = await res.json();
    return body.vip;
}

export async function updateClusterVip(token: string, vip: ClusterVip): Promise<ClusterVip> {
    const res = await authedFetch(`${API_BASE}/cluster/vip`, {
        method: 'PUT',
        headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(vip),
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.status }));
        throw new Error(err.error ?? `Failed to update cluster vip: ${res.status}`);
    }
    const body: { vip: ClusterVip } = await res.json();
    return body.vip;
}
