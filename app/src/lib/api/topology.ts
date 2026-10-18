import { authedFetch } from './http';

const API_BASE = '/api';

export type TopologyNodeKind = 'cluster' | 'agent' | 'workload' | 'volume' | 'resource_group';
export type TopologyEdgeKind = 'member' | 'runs' | 'unscheduled' | 'mounts' | 'attached';

export interface TopologyNode {
    id: string;
    kind: TopologyNodeKind;
    label: string;
    data: Record<string, unknown>;
}

export interface TopologyEdge {
    id: string;
    source: string;
    target: string;
    kind: TopologyEdgeKind;
}

export interface Topology {
    nodes: TopologyNode[];
    edges: TopologyEdge[];
}

export async function getTopology(token: string): Promise<Topology> {
    const res = await authedFetch(`${API_BASE}/topology`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`Failed to load topology: ${res.status}`);
    return res.json();
}
