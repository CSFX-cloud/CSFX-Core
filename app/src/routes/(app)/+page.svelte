<script lang="ts">
    import { untrack } from "svelte";
    import { auth } from "$lib/auth/store.svelte";
    import {
        getClusterStats,
        getHealthHistory,
        getUsageHistory,
        listNodes,
        type ClusterStats,
        type Node,
        type UsageHistoryPoint,
        type UsageRange,
    } from "$lib/api/nodes";
    import { listEvents, type AlertEvent } from "$lib/api/events";
    import {
        listBuckets,
        listVolumes,
        listWorkloads,
        type Bucket,
        type Volume,
        type Workload,
    } from "$lib/api/resource-groups";
    import { clusterUptimePercent, toUsageSample } from "$lib/utils/cluster-metrics";
    import { countServices } from "$lib/utils/dashboard";
    import { Plus } from "@lucide/svelte";
    import AlertsPanel from "$lib/components/dashboard/alerts-panel.svelte";
    import ServersPanel from "$lib/components/dashboard/servers-panel.svelte";
    import ServicesPanel from "$lib/components/dashboard/services-panel.svelte";
    import StatCard from "$lib/components/dashboard/stat-card.svelte";
    import UsagePanel from "$lib/components/dashboard/usage-panel.svelte";

    const LIVE_INTERVAL_MS = 5000;
    const SERVICES_INTERVAL_MS = 10000;
    const UPTIME_INTERVAL_MS = 60000;
    const USAGE_INTERVAL_MS = 30000;

    let stats = $state<ClusterStats | null>(null);
    let usageHistory = $state<UsageHistoryPoint[]>([]);
    let usageRange = $state<UsageRange>("1h");
    let uptimePercent = $state<number | null>(null);
    let nodes = $state<Node[]>([]);
    let workloads = $state<Workload[]>([]);
    let volumes = $state<Volume[]>([]);
    let buckets = $state<Bucket[]>([]);
    let events = $state<AlertEvent[]>([]);
    let loading = $state(true);
    let failures = $state<Record<string, string>>({});

    const error = $derived(Object.values(failures)[0] ?? null);
    const latestUsage = $derived(stats ? toUsageSample(stats) : null);
    const serviceCounts = $derived(countServices(workloads, volumes, buckets));
    const nodeTotal = $derived(stats?.node_count ?? nodes.length);
    const nodesOnline = $derived(stats?.online_count ?? nodes.filter((node) => node.status.toLowerCase() === "online").length);
    const openEvents = $derived(events.filter((event) => event.status === "open"));
    const hasCritical = $derived(openEvents.some((event) => event.severity === "critical"));

    async function guarded(source: string, task: (token: string) => Promise<unknown>) {
        if (!auth.token) return;
        try {
            await task(auth.token);
            delete failures[source];
        } catch (e) {
            failures[source] = e instanceof Error ? e.message : `Failed to load ${source}`;
        }
    }

    const refreshLive = () => guarded("stats", async (token) => (stats = await getClusterStats(token)));
    const refreshNodes = () => guarded("nodes", async (token) => (nodes = await listNodes(token)));
    const refreshEvents = () => guarded("events", async (token) => (events = await listEvents(token)));

    const refreshWorkloads = () => guarded("workloads", async (token) => (workloads = await listWorkloads(token)));
    const refreshVolumes = () => guarded("volumes", async (token) => (volumes = await listVolumes(token)));
    const refreshBuckets = () => guarded("buckets", async (token) => (buckets = await listBuckets(token)));

    const refreshUptime = () =>
        guarded("uptime", async (token) => {
            const weekly = await getHealthHistory(token, "7d");
            uptimePercent = clusterUptimePercent(weekly, stats?.node_count ?? nodes.length);
        });

    const refreshUsage = () =>
        guarded("usage", async (token) => {
            const range = usageRange;
            const history = await getUsageHistory(token, range);
            if (range === usageRange) usageHistory = history;
        });

    $effect(() => {
        if (!auth.token) return;
        const tasks: [() => Promise<void>, number][] = [
            [refreshLive, LIVE_INTERVAL_MS],
            [refreshNodes, SERVICES_INTERVAL_MS],
            [refreshWorkloads, SERVICES_INTERVAL_MS],
            [refreshVolumes, SERVICES_INTERVAL_MS],
            [refreshBuckets, SERVICES_INTERVAL_MS],
            [refreshEvents, SERVICES_INTERVAL_MS],
            [refreshUptime, UPTIME_INTERVAL_MS],
        ];
        untrack(() => {
            Promise.allSettled([refreshNodes(), refreshWorkloads(), refreshVolumes(), refreshBuckets(), refreshEvents()]).then(() => (loading = false));
            refreshLive();
            refreshUptime();
        });
        const timers = tasks.map(([task, interval]) => setInterval(task, interval));
        return () => timers.forEach(clearInterval);
    });

    $effect(() => {
        if (!auth.token) return;
        void usageRange;
        untrack(refreshUsage);
        const timer = setInterval(refreshUsage, USAGE_INTERVAL_MS);
        return () => clearInterval(timer);
    });
</script>

<div class="db-content">
    <section class="cx-stage db-hero" aria-label="Welcome">
        <div class="cx-stage__canvas"></div>
        <div class="db-hero__in">
            <h1>Welcome back{#if auth.user}, {auth.user.username}{/if}.<br /><span style="color: var(--ink-muted)">Your cluster at a glance.</span></h1>
            <div class="db-hero__actions">
                <a class="cx-btn cx-btn--primary" href="/resource-groups">
                    <Plus class="cx-i" aria-hidden="true" />
                    Create
                </a>
            </div>
        </div>
    </section>

    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {/if}

    <section class="db-stats" aria-label="Key figures">
        <StatCard label="Servers online" value={loading ? "-" : String(nodesOnline)} total={loading ? undefined : String(nodeTotal)}>
            {#snippet foot()}
                {#if nodeTotal - nodesOnline > 0}
                    <span class="cx-badge cx-badge--danger">{nodeTotal - nodesOnline} offline</span>
                {:else}
                    <span class="cx-badge cx-badge--success">All online</span>
                {/if}
            {/snippet}
        </StatCard>
        <StatCard label="Uptime · 7d" value={uptimePercent !== null ? `${uptimePercent.toFixed(1)}%` : "-"}>
            {#snippet foot()}Across all servers{/snippet}
        </StatCard>
        <StatCard label="Services" value={loading ? "-" : String(serviceCounts.total)}>
            {#snippet foot()}VMs, containers, volumes, buckets{/snippet}
        </StatCard>
        <StatCard label="Open alerts" value={loading ? "-" : String(openEvents.length)}>
            {#snippet foot()}
                {#if hasCritical}
                    <span class="cx-badge cx-badge--danger">Critical</span>
                {:else if openEvents.length > 0}
                    <span class="cx-badge cx-badge--warning">Warning</span>
                {:else}
                    <span class="cx-badge cx-badge--success">All clear</span>
                {/if}
            {/snippet}
        </StatCard>
    </section>

    <div class="db-grid">
        <div class="db-col">
            <UsagePanel history={usageHistory} latest={latestUsage} range={usageRange} onrange={(range) => (usageRange = range)} />
            <ServersPanel {nodes} metrics={stats?.nodes ?? []} {loading} />
        </div>
        <div class="db-col">
            <ServicesPanel counts={serviceCounts} {loading} />
            <AlertsPanel {events} now={Date.now()} {loading} />
        </div>
    </div>
</div>
