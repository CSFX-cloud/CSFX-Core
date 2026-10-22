<script lang="ts">
    import { onMount, onDestroy, untrack } from "svelte";
    import { SvelteFlow, Background, Controls, type Node, type Edge } from "@xyflow/svelte";
    import "@xyflow/svelte/dist/style.css";
    import { mode } from "mode-watcher";
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
    import * as Sheet from "$lib/components/ui/sheet/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
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
    let detailOpen = $state(false);

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

    async function load() {
        if (!auth.token) return;
        try {
            topology = await getTopology(auth.token);
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
        detailOpen = true;
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
        auth.token;
        load();
    });

    $effect(() => {
        topology;
        visibility;
        highlightedGroup;
        untrack(applyLayout);
    });
</script>



<div class="flex h-svh min-h-0">
    <aside class="hidden shrink-0 flex-col gap-4 overflow-y-auto border-r border-border p-4 pt-8 md:flex" style="width: 16rem;">
        <h2 class="text-xs font-semibold uppercase text-muted-foreground">Layers</h2>
        <label class="flex items-center justify-between text-sm">
            Workloads
            <input type="checkbox" bind:checked={showWorkloads} />
        </label>
        <label class="flex items-center justify-between text-sm {showWorkloads ? '' : 'opacity-50'}">
            Volumes
            <input type="checkbox" bind:checked={showVolumes} disabled={!showWorkloads} />
        </label>

        <h2 class="text-xs font-semibold uppercase text-muted-foreground">Resource groups</h2>
        <div class="flex flex-col gap-1">
            {#each groups as group (group.id)}
                <button
                    class="flex items-center gap-2 rounded px-2 py-1 text-left text-sm hover:bg-muted {highlightedGroup === group.id ? 'bg-muted font-medium' : ''}"
                    onclick={() => (highlightedGroup = highlightedGroup === group.id ? "" : group.id)}
                >
                    <span class="size-2.5 rounded-sm" style="background-color: {group.color}"></span>
                    <span class="truncate">{group.name}</span>
                </button>
            {:else}
                <p class="text-xs text-muted-foreground">No resource groups.</p>
            {/each}
        </div>

        {#if topology}
            <div class="mt-auto text-xs text-muted-foreground">
                {topology.nodes.filter((node) => node.kind === "agent").length} nodes,
                {topology.nodes.filter((node) => node.kind === "workload").length} workloads
            </div>
        {/if}
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
    <main class="relative min-h-0 flex-1">
        <div class="absolute right-4 top-4 z-10 flex items-center gap-3 text-muted-foreground">
            <button type="button" class="hover:text-foreground" title="Reset layout" aria-label="Reset layout" onclick={resetLayout}>
                <RotateCcwIcon class="size-4" />
            </button>
            <button
                type="button"
                class="hover:text-foreground disabled:opacity-50"
                title="Refresh"
                aria-label="Refresh"
                onclick={refreshNow}
                disabled={refreshing}
            >
                <RefreshCwIcon class="size-4 {refreshing ? 'animate-spin' : ''}" />
            </button>
        </div>
        {#if error}
            <p class="absolute left-4 top-16 z-10 text-sm text-destructive">{error}</p>
        {/if}
        {#if nodes.length > 0}
        <SvelteFlow
            bind:nodes
            bind:edges
            nodeTypes={NODE_TYPES}
            colorMode={mode.current ?? "dark"}
            fitView
            nodesConnectable={false}
            deleteKey={null}
            minZoom={0.2}
            proOptions={{ hideAttribution: true }}
            onnodedragstop={onNodeDragStop}
            onnodeclick={onNodeClick}
        >
            <Background />
            <Controls showLock={false} />
        </SvelteFlow>
        {/if}
    </main>
    </div>
</div>

<Sheet.Root bind:open={detailOpen}>
    <Sheet.Content side="right" class="w-full sm:max-w-md overflow-y-auto">
        {#if selectedNode}
            <Sheet.Header>
                <Sheet.Title>{selectedNode.data.label}</Sheet.Title>
            </Sheet.Header>
            <div class="flex flex-col gap-3 px-4 pb-4 text-sm">
                {#each Object.entries(selectedNode.data).filter(([key]) => key !== "label" && key !== "groupColor") as [key, value] (key)}
                    <div class="flex justify-between gap-4 border-b pb-2">
                        <span class="text-muted-foreground">{key}</span>
                        <span class="break-all text-right font-mono text-xs">{value ?? "-"}</span>
                    </div>
                {/each}
                {#if selectedNode.type === "agent"}
                    <div class="flex gap-2 pt-2">
                        <Button href="/nodes/{selectedNode.data.id}" variant="outline" size="sm">Node details</Button>
                        <Button href="/logs?node={selectedNode.data.id}" variant="outline" size="sm">Node logs</Button>
                    </div>
                {/if}
            </div>
        {/if}
    </Sheet.Content>
</Sheet.Root>

<style>
    :global(.svelte-flow__handle) {
        opacity: 0;
    }
</style>
