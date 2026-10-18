import type { Edge, Node } from '@xyflow/svelte';
import { resolveNodeOsIcon, resolveWorkloadIcon } from '$lib/utils/image-icon';
import type { Topology, TopologyEdgeKind, TopologyNode, TopologyNodeKind } from '$lib/api/topology';

export interface TopologyVisibility {
    workloads: boolean;
    volumes: boolean;
}

export interface Point {
    x: number;
    y: number;
}

const NODE_WIDTH: Record<TopologyNodeKind, number> = {
    cluster: 192,
    agent: 240,
    workload: 176,
    volume: 176,
    resource_group: 0,
};

const COLUMN_WIDTH = 680;
const AGENT_ROW_Y = 150;
const CHILD_START_Y = 300;
const CHILD_PITCH = 84;
const CHILD_OFFSET_X = 48;
const MOUNT_OFFSET_X = 240;

const ANIMATED_EDGES = new Set<TopologyEdgeKind>(['runs', 'mounts']);

function isVisible(node: TopologyNode, visibility: TopologyVisibility): boolean {
    if (node.kind === 'resource_group') return false;
    if (node.kind === 'workload') return visibility.workloads;
    if (node.kind === 'volume') return visibility.workloads && visibility.volumes;
    return true;
}

const STATIC_ICONS: Partial<Record<TopologyNodeKind, string>> = {
    cluster: 'mdi:server-network',
    volume: 'mdi:harddisk',
};

function nodeIcon(node: TopologyNode): string {
    if (node.kind === 'agent') return resolveNodeOsIcon(String(node.data.os_type ?? ''));
    if (node.kind === 'workload') {
        return resolveWorkloadIcon(String(node.data.image ?? ''), String(node.data.runtime_class ?? ''));
    }
    return STATIC_ICONS[node.kind] ?? 'mdi:circle-outline';
}

function toFlowNode(node: TopologyNode, groupColors: Map<string, string>): Node {
    const groupId = node.data.resource_group_id as string | null | undefined;
    return {
        id: node.id,
        type: node.kind,
        position: { x: 0, y: 0 },
        data: { label: node.label, icon: nodeIcon(node), ...node.data, groupColor: groupId ? groupColors.get(groupId) : undefined },
    };
}

type TopologyEdgeData = Topology['edges'][number];

interface Row {
    edge: TopologyEdgeData;
    mounts: TopologyEdgeData[];
}

interface Column {
    parentId: string;
    rows: Row[];
}

const CLUSTER_ID = 'cluster';
const UNASSIGNED_ID = 'unassigned';

function flowEdge(id: string, source: string, target: string, kind: TopologyEdgeKind, extra: Partial<Edge> = {}): Edge {
    return {
        id,
        source,
        target,
        type: 'smoothstep',
        animated: ANIMATED_EDGES.has(kind),
        style: kind === 'unscheduled' || kind === 'unattached' ? 'stroke-dasharray: 6 4' : undefined,
        ...extra,
    };
}

function childrenByParent(edges: TopologyEdgeData[]): Map<string, TopologyEdgeData[]> {
    const grouped = new Map<string, TopologyEdgeData[]>();
    for (const edge of edges) {
        grouped.set(edge.source, [...(grouped.get(edge.source) ?? []), edge]);
    }
    return grouped;
}

function sortedByLabel(edges: TopologyEdgeData[], labels: Map<string, string>): TopologyEdgeData[] {
    return [...edges].sort((a, b) => (labels.get(a.target) ?? '').localeCompare(labels.get(b.target) ?? ''));
}

function rowsOf(
    direct: TopologyEdgeData[],
    children: Map<string, TopologyEdgeData[]>,
    labels: Map<string, string>,
): Row[] {
    return sortedByLabel(direct, labels).map((edge) => ({
        edge,
        mounts: sortedByLabel(children.get(edge.target) ?? [], labels),
    }));
}

function columnsOf(topology: Topology, edges: TopologyEdgeData[], visibleIds: Set<string>): Column[] {
    const labels = new Map(topology.nodes.map((node) => [node.id, node.label]));
    const children = childrenByParent(edges);
    const agents = topology.nodes
        .filter((node) => node.kind === 'agent' && visibleIds.has(node.id))
        .sort((a, b) => a.label.localeCompare(b.label));
    const agentIds = new Set(agents.map((agent) => agent.id));
    const columns = agents.map((agent) => ({
        parentId: agent.id,
        rows: rowsOf(children.get(agent.id) ?? [], children, labels),
    }));
    const loose = (children.get(CLUSTER_ID) ?? []).filter((edge) => !agentIds.has(edge.target));
    return loose.length > 0
        ? [...columns, { parentId: UNASSIGNED_ID, rows: rowsOf(loose, children, labels) }]
        : columns;
}

function placeColumn(column: Column, index: number, positions: Map<string, Point>) {
    const centerX = index * COLUMN_WIDTH;
    const parentWidth = column.parentId === UNASSIGNED_ID ? NODE_WIDTH.cluster : NODE_WIDTH.agent;
    positions.set(column.parentId, { x: centerX - parentWidth / 2, y: AGENT_ROW_Y });
    column.rows.forEach((row, rowIndex) => {
        const y = CHILD_START_Y + rowIndex * CHILD_PITCH;
        const childX = centerX + CHILD_OFFSET_X;
        positions.set(row.edge.target, { x: childX, y });
        row.mounts.forEach((mount, mountIndex) => {
            positions.set(mount.target, { x: childX + MOUNT_OFFSET_X * (mountIndex + 1), y });
        });
    });
}

function columnEdges(column: Column): Edge[] {
    return column.rows.flatMap((row) => [
        flowEdge(row.edge.id, column.parentId, row.edge.target, row.edge.kind, { targetHandle: 'left' }),
        ...row.mounts.map((mount, index) =>
            flowEdge(
                mount.id,
                index === 0 ? row.edge.target : row.mounts[index - 1].target,
                mount.target,
                mount.kind,
                { type: 'straight', sourceHandle: 'right', targetHandle: 'left' },
            ),
        ),
    ]);
}

function unassignedNode(position: Point): Node {
    return {
        id: UNASSIGNED_ID,
        type: UNASSIGNED_ID,
        position,
        data: { label: 'Unassigned', icon: 'mdi:help-box-outline' },
    };
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
    const treeEdges = topology.edges.filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target));

    const columns = columnsOf(topology, treeEdges, visibleIds);
    const positions = new Map<string, Point>();
    columns.forEach((column, index) => placeColumn(column, index, positions));

    const centerX = ((columns.length - 1) * COLUMN_WIDTH) / 2;
    positions.set(CLUSTER_ID, { x: centerX - NODE_WIDTH.cluster / 2, y: 0 });

    const memberEdges = treeEdges
        .filter((edge) => edge.kind === 'member')
        .map((edge) => flowEdge(edge.id, edge.source, edge.target, edge.kind));
    const hasUnassigned = columns.some((column) => column.parentId === UNASSIGNED_ID);
    const unassignedEdges = hasUnassigned
        ? [flowEdge(`${CLUSTER_ID}:${UNASSIGNED_ID}`, CLUSTER_ID, UNASSIGNED_ID, 'unscheduled')]
        : [];

    const nodes = visible.map((node) => ({
        ...toFlowNode(node, groupColors),
        position: manualPositions.get(node.id) ?? positions.get(node.id) ?? { x: 0, y: 0 },
    }));
    if (hasUnassigned) {
        const position = manualPositions.get(UNASSIGNED_ID) ?? positions.get(UNASSIGNED_ID) ?? { x: 0, y: 0 };
        nodes.push(unassignedNode(position));
    }

    return { nodes, edges: [...memberEdges, ...unassignedEdges, ...columns.flatMap(columnEdges)] };
}
