<script lang="ts">
    import { untrack } from "svelte";
    import { Plus, RefreshCw } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getClusterStats, listNodes, type ClusterStats, type Node } from "$lib/api/nodes";
    import AddNodeDialog from "$lib/components/nodes/add-node-dialog.svelte";
    import NodesOverviewStats from "$lib/components/nodes/nodes-overview-stats.svelte";
    import NodesTable from "$lib/components/nodes/nodes-table.svelte";

    const STATS_INTERVAL_MS = 10_000;

    let nodes = $state<Node[]>([]);
    let stats = $state<ClusterStats | null>(null);
    let loading = $state(true);
    let refreshing = $state(false);
    let error = $state<string | null>(null);
    let addNodeDialog = $state<ReturnType<typeof AddNodeDialog> | null>(null);

    async function refreshStats() {
        if (!auth.token) return;
        try {
            stats = await getClusterStats(auth.token);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load cluster stats";
        }
    }

    async function refresh() {
        if (!auth.token || refreshing) return;
        refreshing = true;
        try {
            [nodes] = await Promise.all([listNodes(auth.token), refreshStats()]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load nodes";
        } finally {
            refreshing = false;
            loading = false;
        }
    }

    $effect(() => {
        if (!auth.token) return;
        untrack(refresh);
        const timer = setInterval(refreshStats, STATS_INTERVAL_MS);
        return () => clearInterval(timer);
    });
</script>

<div class="db-content">
    <section class="cx-stage db-hero" aria-label="Nodes">
        <div class="cx-stage__canvas"></div>
        <div class="db-hero__in">
            <h1>Nodes.<br /><span style="color: var(--ink-muted)">Every hypervisor in this cluster.</span></h1>
            <div class="db-actions">
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={refreshing} aria-busy={refreshing} onclick={refresh}>
                    <RefreshCw class="cx-i {refreshing ? 'db-spin' : ''}" aria-hidden="true" />
                    Refresh
                </button>
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => addNodeDialog?.open()}>
                    <Plus class="cx-i" aria-hidden="true" />
                    Add node
                </button>
            </div>
        </div>
    </section>

    <NodesOverviewStats {stats} />
    <NodesTable {nodes} metrics={stats?.nodes ?? []} {loading} {error} />
</div>

<AddNodeDialog bind:this={addNodeDialog} />
