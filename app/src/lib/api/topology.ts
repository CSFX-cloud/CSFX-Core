import { api } from './http';

export type TopologyNodeKind = 'cluster' | 'agent' | 'workload' | 'volume' | 'resource_group';
export type TopologyEdgeKind = 'member' | 'runs' | 'unscheduled' | 'mounts' | 'attached' | 'unattached';

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

export const getTopology = () =>
    api<Topology>('GET', '/topology');
