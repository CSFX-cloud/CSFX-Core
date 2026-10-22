<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { page } from "$app/state";
    import { auth } from "$lib/auth/store.svelte";
    import {
        exportLogsCef,
        getLogsFacets,
        listLogs,
        type LogEntry,
        type LogsFacets,
        type LogsFilter,
    } from "$lib/api/logs";
    import { listNodes, type Node } from "$lib/api/nodes";
    import { listUsers, type OrganizationUser } from "$lib/api/users";
    import { csv, describeAction, levelTextClass, rangeStart, type RangeKey } from "$lib/utils/logs";
    import LogsFilterPanel from "$lib/components/logs/logs-filter-panel.svelte";
    import LogDetailPanel from "$lib/components/logs/log-detail-panel.svelte";
    import SeverityBars from "$lib/components/logs/severity-bars.svelte";

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
            const [list, facetResult] = await Promise.all([
                listLogs(auth.token, { ...filter, limit: PAGE_LIMIT, offset }),
                getLogsFacets(auth.token, filter),
            ]);
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

    function targetLabel(entry: LogEntry): string {
        if (!entry.target_id || entry.target_id === "-") return "-";
        return entry.target_id;
    }

    function formatDateTime(timestamp: string): string {
        return new Date(timestamp).toLocaleString();
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

<div class="flex h-svh min-h-0">
    <aside class="hidden shrink-0 overflow-y-auto border-r border-border md:block" style="width: 16rem;">
        <LogsFilterPanel
            bind:kind
            bind:range
            bind:levels
            bind:classifications
            bind:services
            bind:targetTypes
            bind:actions
            bind:agentId
            bind:searchText
            bind:fromTime
            bind:toTime
            {facets}
            {nodes}
            {refreshing}
            onrefresh={refreshNow}
            onclear={clearFilters}
            onexport={exportCef}
        />
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
        {#if error}
            <p class="px-4 pb-2 text-sm text-destructive">{error}</p>
        {/if}

        <div class="min-h-0 flex-1 overflow-y-auto">
            <table class="w-full border-collapse text-sm">
                <thead class="sticky top-0 z-10 bg-background">
                    {#if kind === "system"}
                        <tr class="text-left text-xs text-muted-foreground">
                            <th class="px-3 py-2 font-medium">Severity</th>
                            <th class="px-3 py-2 font-medium">Category</th>
                            <th class="px-3 py-2 font-medium">Service</th>
                            <th class="px-3 py-2 font-medium">Node</th>
                            <th class="px-3 py-2 font-medium">Description</th>
                            <th class="whitespace-nowrap px-3 py-2 font-medium">Time</th>
                        </tr>
                    {:else}
                        <tr class="text-left text-xs text-muted-foreground">
                            <th class="whitespace-nowrap px-3 py-2 font-medium">Time</th>
                            <th class="px-3 py-2 font-medium">User</th>
                            <th class="px-3 py-2 font-medium">Event</th>
                            <th class="px-3 py-2 font-medium">Action</th>
                            <th class="px-3 py-2 font-medium">Resource</th>
                            <th class="px-3 py-2 font-medium">Target</th>
                        </tr>
                    {/if}
                </thead>
                <tbody>
                    {#if loading}
                        <tr><td class="px-3 py-3 text-muted-foreground" colspan="6">Loading...</td></tr>
                    {:else if entries.length === 0}
                        <tr><td class="px-3 py-3 text-muted-foreground" colspan="6">No log entries.</td></tr>
                    {:else}
                        {#each entries as entry (entry.id)}
                            <tr
                                class="cursor-pointer hover:bg-muted/30 {selectedEntry?.id === entry.id ? 'bg-muted/50' : ''}"
                                onclick={() => (selectedEntry = entry)}
                            >
                                {#if kind === "system"}
                                    <td class="px-3 py-1.5"><SeverityBars level={entry.level} /></td>
                                    <td class="whitespace-nowrap px-3 py-1.5">{entry.classification}</td>
                                    <td class="whitespace-nowrap px-3 py-1.5">{entry.service}</td>
                                    <td class="whitespace-nowrap px-3 py-1.5">{nodeLabel(entry.agent_id)}</td>
                                    <td class="max-w-0 truncate px-3 py-1.5">
                                        <span class={entry.level === "ERROR" ? levelTextClass(entry.level) : ""}>{entry.message}</span>
                                        {#if entry.occurrences > 1}
                                            <span class="ms-1 rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">x{entry.occurrences}</span>
                                        {/if}
                                    </td>
                                    <td class="whitespace-nowrap px-3 py-1.5 text-muted-foreground">{formatDateTime(entry.last_seen_at)}</td>
                                {:else}
                                    <td class="whitespace-nowrap px-3 py-1.5 text-muted-foreground">{formatDateTime(entry.created_at)}</td>
                                    <td class="whitespace-nowrap px-3 py-1.5">{actorLabel(entry.actor_user_id)}</td>
                                    <td class="whitespace-nowrap px-3 py-1.5">{describeAction(entry)}</td>
                                    <td class="whitespace-nowrap px-3 py-1.5 font-mono text-xs">{entry.action ?? entry.message}</td>
                                    <td class="whitespace-nowrap px-3 py-1.5">{entry.target_type ?? "-"}</td>
                                    <td class="max-w-0 truncate px-3 py-1.5 font-mono text-xs">{targetLabel(entry)}</td>
                                {/if}
                            </tr>
                        {/each}
                    {/if}
                </tbody>
            </table>
        </div>

        <div class="flex items-center justify-between border-t border-border px-4 py-2 text-xs text-muted-foreground">
            <span>{entries.length === 0 ? 0 : offset + 1}-{offset + entries.length} of {total} logs</span>
            <div class="flex gap-2">
                <button class="rounded border px-2 py-1 disabled:opacity-40" onclick={() => (offset = Math.max(0, offset - PAGE_LIMIT))} disabled={offset === 0}>Prev</button>
                <button class="rounded border px-2 py-1 disabled:opacity-40" onclick={() => (offset += PAGE_LIMIT)} disabled={!hasMore}>Next</button>
            </div>
        </div>
    </div>

    {#if selectedEntry}
        <aside class="hidden shrink-0 border-l border-border lg:block" style="width: 24rem;">
            <LogDetailPanel entry={selectedEntry} {nodeLabel} {actorLabel} onclose={() => (selectedEntry = null)} />
        </aside>
    {/if}
</div>
