<script lang="ts">
    import { onMount, onDestroy, untrack } from "svelte";
    import { SvelteFlow, Background, Controls, type Node, type Edge } from "@xyflow/svelte";
    import "@xyflow/svelte/dist/style.css";
    import { theme } from "$lib/utils/theme.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getTopology, type Topology } from "$lib/api/topology";
    import {
        layoutTopology,
        resourceGroupColors,
        type Point,
        type TopologyVisibility,
    } from "$lib/utils/topology-layout";
    import AgentNode from "$lib/components/monitoring/agent-node.svelte";
    import CompactNode from "$lib/components/monitoring/compact-node.svelte";
    import TopologyDrawer from "$lib/components/monitoring/topology-drawer.svelte";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import RotateCcwIcon from "@lucide/svelte/icons/rotate-ccw";

    const REFRESH_INTERVAL_MS = 7000;
    const NODE_TYPES = {
        cluster: CompactNode,
        unassigned: CompactNode,
        agent: AgentNode,
        workload: CompactNode,
        volume: CompactNode,
    };

    let topology = $state<Topology | null>(null);
    let nodes = $state.raw<Node[]>([]);
    let edges = $state.raw<Edge[]>([]);
    let error = $state<string | null>(null);
    let refreshing = $state(false);
    let refreshTimer: ReturnType<typeof setInterval> | null = null;

    let showWorkloads = $state(true);
    let showVolumes = $state(false);
    let highlightedGroup = $state("");

    let structureKey = "";

    let selectedNode = $state<Node | null>(null);

    const manualPositions = new Map<string, Point>();

    const visibility = $derived<TopologyVisibility>({
        workloads: showWorkloads,
        volumes: showVolumes,
    });
    const groups = $derived(
        topology
            ? topology.nodes
                  .filter((node) => node.kind === "resource_group")
                  .map((node) => ({
                      id: node.data.id as string,
                      name: node.label,
                      color: node.data.color as string,
                  }))
            : [],
    );
    const groupColors = $derived(topology ? resourceGroupColors(topology) : new Map());
    const groupNames = $derived(new Map(groups.map((group) => [group.id, group.name])));

    async function load() {
        try {
            topology = await getTopology();
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load topology";
        } finally {
            refreshing = false;
        }
    }

    function refreshNow() {
        refreshing = true;
        load();
    }

    function dimmed(node: Node): boolean {
        if (!highlightedGroup) return false;
        return node.type === "workload" && node.data.resource_group_id !== highlightedGroup;
    }

    function structureOf(result: { nodes: Node[]; edges: Edge[] }): string {
        return [...result.nodes.map((node) => node.id), ...result.edges.map((edge) => edge.id)].join("|");
    }

    function refreshData(fresh: Node[]) {
        const byId = new Map(fresh.map((node) => [node.id, node]));
        nodes = nodes.map((node) => {
            const next = byId.get(node.id);
            return next ? { ...node, data: next.data, class: dimmed(next) ? "opacity-30" : "" } : node;
        });
    }

    function applyLayout() {
        if (!topology) return;
        const result = layoutTopology(topology, visibility, manualPositions);
        const key = structureOf(result);
        if (key === structureKey) {
            refreshData(result.nodes);
            return;
        }
        structureKey = key;
        nodes = result.nodes.map((node) => ({ ...node, class: dimmed(node) ? "opacity-30" : "" }));
        edges = result.edges;
    }

    function onNodeDragStop({ targetNode }: { targetNode: Node | null }) {
        if (targetNode) manualPositions.set(targetNode.id, targetNode.position);
    }

    function onNodeClick({ node }: { node: Node }) {
        if (node.type === "cluster") return;
        selectedNode = node;
    }

    function resetLayout() {
        manualPositions.clear();
        structureKey = "";
        applyLayout();
    }

    onMount(() => {
        refreshTimer = setInterval(load, REFRESH_INTERVAL_MS);
    });

    onDestroy(() => {
        if (refreshTimer) clearInterval(refreshTimer);
    });

    $effect(() => {
        auth.user;
        load();
    });

    $effect(() => {
        topology;
        visibility;
        highlightedGroup;
        untrack(applyLayout);
    });
</script>



<div class="tp-main">
    <aside class="tp-side" aria-label="Map options">
        <div>
            <h2>Layers</h2>
            <div class="tp-opts">
                <label class="cx-check"><input type="checkbox" bind:checked={showWorkloads} /> Workloads</label>
                <label class="cx-check" style={showWorkloads ? "" : "opacity: .5"}>
                    <input type="checkbox" bind:checked={showVolumes} disabled={!showWorkloads} /> Volumes
                </label>
            </div>
        </div>
        <div>
            <h2>Resource groups</h2>
            {#each groups as group (group.id)}
                <button
                    class="tp-group"
                    type="button"
                    aria-pressed={highlightedGroup === group.id}
                    onclick={() => (highlightedGroup = highlightedGroup === group.id ? "" : group.id)}
                >
                    <span class="tp-dot" style="background: {group.color}"></span>
                    <span class="truncate">{group.name}</span>
                </button>
            {:else}
                <p class="cx-hint">No resource groups.</p>
            {/each}
        </div>
        {#if topology}
            <p class="tp-foot">
                {topology.nodes.filter((node) => node.kind === "agent").length} nodes &middot;
                {topology.nodes.filter((node) => node.kind === "workload").length} workloads
            </p>
        {/if}
    </aside>

    <section class="tp-canvas" aria-label="Cluster map">
        <div class="tp-tools">
            <button class="cx-icon-btn cx-icon-btn--sm" type="button" title="Reset layout" aria-label="Reset layout" onclick={resetLayout}>
                <RotateCcwIcon class="cx-i" />
            </button>
            <button class="cx-icon-btn cx-icon-btn--sm" type="button" title="Refresh" aria-label="Refresh" disabled={refreshing} onclick={refreshNow}>
                <RefreshCwIcon class="cx-i {refreshing ? 'db-spin' : ''}" />
            </button>
        </div>
        {#if error}<p class="cx-hint cx-hint--error tp-error">{error}</p>{/if}
        {#if nodes.length > 0}
            <SvelteFlow
                bind:nodes
                bind:edges
                nodeTypes={NODE_TYPES}
                colorMode={theme.resolved}
                fitView
                nodesConnectable={false}
                deleteKey={null}
                minZoom={0.2}
                proOptions={{ hideAttribution: true }}
                onnodedragstop={onNodeDragStop}
                onnodeclick={onNodeClick}
            >
                <Background gap={22} size={1.3} />
                <Controls showLock={false} />
            </SvelteFlow>
        {/if}
        <div class="tp-legend" aria-hidden="true">
            <span><i style="background: var(--success)"></i>Online</span>
            <span><i style="background: var(--danger)"></i>Offline</span>
        </div>
    </section>
</div>

<TopologyDrawer node={selectedNode} {groupNames} onclose={() => (selectedNode = null)} />

<style>
    :global(.svelte-flow__handle) {
        opacity: 0;
    }
</style>
