<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { getClusterStats, getHealthHistory, listNodes, type ClusterStats, type Node } from "$lib/api/nodes";
    import {
        listBuckets,
        listVolumes,
        listWorkloads,
        type Bucket,
        type Volume,
        type Workload,
    } from "$lib/api/resource-groups";
    import { clusterUptimePercent, toUsageSample, uptimeColor, type UsageSample } from "$lib/utils/cluster-metrics";
    import { summarizeNodes, summarizeServices } from "$lib/utils/dashboard";
    import SegmentCard from "$lib/components/dashboard/segment-card.svelte";
    import DashboardInfoPanel from "$lib/components/dashboard/dashboard-info-panel.svelte";
    import SearchHeader from "$lib/components/search-header.svelte";

    const INFO_PANEL_WIDTH = "16rem";
    const LIVE_INTERVAL_MS = 5000;
    const SERVICES_INTERVAL_MS = 10000;
    const UPTIME_INTERVAL_MS = 60000;
    const USAGE_CAPACITY = 60;

    let stats = $state<ClusterStats | null>(null);
    let usage = $state<UsageSample[]>([]);
    let uptimePercent = $state<number | null>(null);
    let dailyUptimePercent = $state<number | null>(null);
    let nodes = $state<Node[]>([]);
    let workloads = $state<Workload[]>([]);
    let volumes = $state<Volume[]>([]);
    let buckets = $state<Bucket[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);

    const nodeSegments = $derived(summarizeNodes(nodes));
    const serviceSegments = $derived(summarizeServices(workloads, volumes, buckets));

    function report(e: unknown) {
        error = e instanceof Error ? e.message : "Failed to load dashboard";
    }

    async function refreshLive() {
        if (!auth.token) return;
        try {
            stats = await getClusterStats(auth.token);
            usage = [...usage, toUsageSample(stats)].slice(-USAGE_CAPACITY);
            error = null;
        } catch (e) {
            report(e);
        }
    }

    async function refreshServices() {
        if (!auth.token) return;
        try {
            [nodes, workloads, volumes, buckets] = await Promise.all([
                listNodes(auth.token),
                listWorkloads(auth.token),
                listVolumes(auth.token),
                listBuckets(auth.token),
            ]);
            error = null;
        } catch (e) {
            report(e);
        } finally {
            loading = false;
        }
    }

    async function refreshUptime() {
        if (!auth.token) return;
        try {
            const nodeCount = stats?.node_count ?? nodes.length;
            const [weekly, daily] = await Promise.all([
                getHealthHistory(auth.token, "7d"),
                getHealthHistory(auth.token, "24h"),
            ]);
            uptimePercent = clusterUptimePercent(weekly, nodeCount);
            dailyUptimePercent = clusterUptimePercent(daily, nodeCount);
        } catch (e) {
            report(e);
        }
    }

    $effect(() => {
        if (!auth.token) return;
        const tasks: [() => Promise<void>, number][] = [
            [refreshLive, LIVE_INTERVAL_MS],
            [refreshServices, SERVICES_INTERVAL_MS],
            [refreshUptime, UPTIME_INTERVAL_MS],
        ];
        tasks.forEach(([task]) => task());
        const timers = tasks.map(([task, interval]) => setInterval(task, interval));
        return () => timers.forEach(clearInterval);
    });
</script>

<div class="flex min-h-0 flex-1">
    <div class="hidden shrink-0 border-r border-border md:block pt-4" style="width: {INFO_PANEL_WIDTH};">
        <DashboardInfoPanel {uptimePercent} {usage} usageCapacity={USAGE_CAPACITY} />
    </div>

    <div class="flex min-w-0 flex-1 flex-col">
        <SearchHeader />

        <div class="flex flex-1 flex-col gap-6 p-6">
            <div>
                <h1 class="text-2xl font-semibold tracking-tight">
                    Welcome back{#if auth.user}, <span class="text-primary">{auth.user.username}</span>{/if}
                </h1>
                <p class="text-sm text-muted-foreground mt-1">
                    Global uptime at
                    <span class="font-medium" style="color: {uptimeColor(dailyUptimePercent)}">{dailyUptimePercent !== null ? `${dailyUptimePercent.toFixed(2)}%` : "-"}</span>
                    in the last 24h.
                </p>
            </div>
            {#if error}
                <p class="text-sm text-destructive">{error}</p>
            {/if}
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
                <SegmentCard title="Servers" totalLabel="Servers" segments={nodeSegments} {loading} />
                <SegmentCard title="Services" totalLabel="Services" segments={serviceSegments} {loading} />
            </div>
        </div>
    </div>
</div>
