<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { Download, RefreshCw, Search } from "@lucide/svelte";
    import { page } from "$app/state";
    import { auth } from "$lib/auth/store.svelte";
    import {
        exportLogsCef,
        getLogsFacets,
        getLogsTrend,
        listLogs,
        type LogEntry,
        type LogsFacets,
        type LogsFilter,
        type LogsTrendPoint,
    } from "$lib/api/logs";
    import { listNodes, type Node } from "$lib/api/nodes";
    import { listUsers, type OrganizationUser } from "$lib/api/users";
    import { csv, rangeStart, type RangeKey } from "$lib/utils/logs";
    import LogsFilterPanel from "$lib/components/logs/logs-filter-panel.svelte";
    import LogDetailPanel from "$lib/components/logs/log-detail-panel.svelte";
    import LogsTable from "$lib/components/logs/logs-table.svelte";
    import LogsTrend from "$lib/components/logs/logs-trend.svelte";

    type LogKind = "user" | "system";

    const REFRESH_INTERVAL_MS = 7000;
    const PAGE_LIMIT = 100;
    const initialNode = page.url.searchParams.get("node") ?? "";

    let kind = $state<LogKind>("system");
    let range = $state<RangeKey>("1D");
    let levels = $state<string[]>([]);
    let classifications = $state<string[]>([]);
    let services = $state<string[]>([]);
    let targetTypes = $state<string[]>([]);
    let actions = $state<string[]>([]);
    let agentId = $state(initialNode);
    let searchText = $state("");
    let fromTime = $state("");
    let toTime = $state("");

    let entries = $state<LogEntry[]>([]);
    let facets = $state<LogsFacets | null>(null);
    let trend = $state<LogsTrendPoint[]>([]);
    let total = $state(0);
    let hasMore = $state(false);
    let offset = $state(0);
    let loading = $state(true);
    let refreshing = $state(false);
    let error = $state<string | null>(null);
    let refreshTimer: ReturnType<typeof setInterval> | null = null;

    let nodes = $state<Node[]>([]);
    let users = $state<OrganizationUser[]>([]);
    let selectedEntry = $state<LogEntry | null>(null);

    const nodeNames = $derived(new Map(nodes.map((node) => [node.id, node.name])));
    const userNames = $derived(new Map(users.map((user) => [user.id, user.username])));

    function baseFilter(): LogsFilter {
        const custom = range === "custom";
        return {
            kind,
            level: csv(levels),
            classification: kind === "system" ? csv(classifications) : undefined,
            service: kind === "system" ? csv(services) : undefined,
            agent_id: kind === "system" ? agentId || undefined : undefined,
            target_type: kind === "user" ? csv(targetTypes) : undefined,
            action: kind === "user" ? csv(actions) : undefined,
            q: searchText || undefined,
            from: custom ? (fromTime ? new Date(fromTime).toISOString() : undefined) : rangeStart(range),
            to: custom && toTime ? new Date(toTime).toISOString() : undefined,
        };
    }

    async function load() {
        if (!auth.token) return;
        try {
            const filter = baseFilter();
            const [list, facetResult, trendResult] = await Promise.all([
                listLogs(auth.token, { ...filter, limit: PAGE_LIMIT, offset }),
                getLogsFacets(auth.token, filter),
                getLogsTrend(auth.token, filter),
            ]);
            trend = trendResult;
            entries = list.entries;
            total = list.total;
            hasMore = list.has_more;
            facets = facetResult;
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load logs";
        } finally {
            loading = false;
            refreshing = false;
        }
    }

    async function loadDirectories() {
        if (!auth.token) return;
        const [nodeResult, userResult] = await Promise.allSettled([listNodes(auth.token), listUsers(auth.token)]);
        if (nodeResult.status === "fulfilled") nodes = nodeResult.value;
        if (userResult.status === "fulfilled") users = userResult.value;
    }

    function refreshNow() {
        refreshing = true;
        load();
    }

    function clearFilters() {
        levels = [];
        classifications = [];
        services = [];
        targetTypes = [];
        actions = [];
        agentId = "";
        searchText = "";
        fromTime = "";
        toTime = "";
        range = "1D";
    }

    async function exportCef() {
        if (!auth.token) return;
        try {
            await exportLogsCef(auth.token, baseFilter());
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to export logs";
        }
    }

    function nodeLabel(id: string | null): string {
        if (!id) return "-";
        return nodeNames.get(id) ?? id.slice(0, 8);
    }

    function actorLabel(id: string | null): string {
        if (!id) return "-";
        return userNames.get(id) ?? id.slice(0, 8);
    }

    onMount(() => {
        loadDirectories();
        refreshTimer = setInterval(load, REFRESH_INTERVAL_MS);
    });

    onDestroy(() => {
        if (refreshTimer) clearInterval(refreshTimer);
    });

    $effect(() => {
        auth.token;
        kind;
        range;
        levels;
        classifications;
        services;
        targetTypes;
        actions;
        agentId;
        searchText;
        fromTime;
        toTime;
        offset;
        load();
    });

    $effect(() => {
        kind;
        range;
        levels;
        classifications;
        services;
        targetTypes;
        actions;
        agentId;
        searchText;
        fromTime;
        toTime;
        offset = 0;
        selectedEntry = null;
    });
</script>

<div class="lg-main {selectedEntry ? 'lg-main--detail' : ''}">
    <LogsFilterPanel
        bind:kind
        bind:range
        bind:levels
        bind:classifications
        bind:services
        bind:targetTypes
        bind:actions
        bind:agentId
        bind:fromTime
        bind:toTime
        {facets}
        {nodes}
        onclear={clearFilters}
    />

    <div class="lg-body">
        <div class="lg-head">
            <div>
                <h1 class="db-title">Logs</h1>
                <p class="db-sub">Every event from the cluster, stored on the cluster.</p>
            </div>
            <div class="db-actions">
                <div class="cx-input-wrap db-search">
                    <Search class="cx-i" aria-hidden="true" />
                    <input class="cx-input cx-input--icon" type="search" placeholder="Search messages" aria-label="Search messages" bind:value={searchText} />
                </div>
                <button class="cx-icon-btn lg-calendar" type="button" aria-label="Refresh" disabled={refreshing} onclick={refreshNow}>
                    <RefreshCw class="cx-i {refreshing ? 'db-spin' : ''}" aria-hidden="true" />
                </button>
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={exportCef}>
                    <Download class="cx-i" aria-hidden="true" />
                    Export as CEF
                </button>
            </div>
        </div>

        {#if error}<p class="cx-hint cx-hint--error" style="margin-bottom: 8px">{error}</p>{/if}

        <LogsTrend points={trend} />

        <LogsTable
            {kind}
            {entries}
            selectedId={selectedEntry?.id ?? null}
            {loading}
            {offset}
            {total}
            pageLimit={PAGE_LIMIT}
            {hasMore}
            {nodeLabel}
            {actorLabel}
            onselect={(entry) => (selectedEntry = entry)}
            onpage={(next) => (offset = next)}
        />
    </div>

    {#if selectedEntry}
        <LogDetailPanel entry={selectedEntry} {nodeLabel} {actorLabel} onclose={() => (selectedEntry = null)} />
    {/if}
</div>
