import dagre from '@dagrejs/dagre';
import type { Edge, Node } from '@xyflow/svelte';
import type { Topology, TopologyEdgeKind, TopologyNode, TopologyNodeKind } from '$lib/api/topology';

export interface TopologyVisibility {
    workloads: boolean;
    volumes: boolean;
}

export interface Point {
    x: number;
    y: number;
}

const NODE_SIZE: Record<TopologyNodeKind, { width: number; height: number }> = {
    cluster: { width: 200, height: 64 },
    agent: { width: 240, height: 96 },
    workload: { width: 190, height: 56 },
    volume: { width: 170, height: 48 },
    resource_group: { width: 0, height: 0 },
};

const ANIMATED_EDGES = new Set<TopologyEdgeKind>(['runs', 'mounts']);

function isVisible(node: TopologyNode, visibility: TopologyVisibility): boolean {
    if (node.kind === 'resource_group') return false;
    if (node.kind === 'workload') return visibility.workloads;
    if (node.kind === 'volume') return visibility.workloads && visibility.volumes;
    return true;
}

function toFlowNode(node: TopologyNode, groupColors: Map<string, string>): Node {
    const groupId = node.data.resource_group_id as string | null | undefined;
    return {
        id: node.id,
        type: node.kind,
        position: { x: 0, y: 0 },
        data: { label: node.label, ...node.data, groupColor: groupId ? groupColors.get(groupId) : undefined },
    };
}

function toFlowEdge(edge: Topology['edges'][number]): Edge {
    return {
        id: edge.id,
        source: edge.source,
        target: edge.target,
        type: 'smoothstep',
        animated: ANIMATED_EDGES.has(edge.kind),
        style: edge.kind === 'unscheduled' ? 'stroke-dasharray: 6 4' : undefined,
    };
}

function applyDagre(nodes: Node[], edges: Edge[]): Map<string, Point> {
    const graph = new dagre.graphlib.Graph();
    graph.setGraph({ rankdir: 'TB', nodesep: 40, ranksep: 90 });
    graph.setDefaultEdgeLabel(() => ({}));

    for (const node of nodes) {
        const size = NODE_SIZE[node.type as TopologyNodeKind];
        graph.setNode(node.id, size);
    }
    for (const edge of edges) graph.setEdge(edge.source, edge.target);

    dagre.layout(graph);

    return new Map(
        nodes.map((node) => {
            const { x, y, width, height } = graph.node(node.id);
            return [node.id, { x: x - width / 2, y: y - height / 2 }];
        }),
    );
}

export function resourceGroupColors(topology: Topology): Map<string, string> {
    return new Map(
        topology.nodes
            .filter((node) => node.kind === 'resource_group')
            .map((node) => [node.data.id as string, node.data.color as string]),
    );
}

export function layoutTopology(
    topology: Topology,
    visibility: TopologyVisibility,
    manualPositions: Map<string, Point>,
): { nodes: Node[]; edges: Edge[] } {
    const groupColors = resourceGroupColors(topology);
    const visible = topology.nodes.filter((node) => isVisible(node, visibility));
    const visibleIds = new Set(visible.map((node) => node.id));

    const nodes = visible.map((node) => toFlowNode(node, groupColors));
    const edges = topology.edges
        .filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target))
        .map(toFlowEdge);

    const automatic = applyDagre(nodes, edges);
    for (const node of nodes) {
        node.position = manualPositions.get(node.id) ?? automatic.get(node.id) ?? node.position;
    }

    return { nodes, edges };
}
