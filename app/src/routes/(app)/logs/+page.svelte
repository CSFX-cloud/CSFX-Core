<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { page } from "$app/state";
    import { auth } from "$lib/auth/store.svelte";
    import { listLogs, type LogEntry, type LogsFilter } from "$lib/api/logs";
    import { listNodes, type Node } from "$lib/api/nodes";
    import { listUsers, type OrganizationUser } from "$lib/api/users";
    import * as Sheet from "$lib/components/ui/sheet/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import SearchHeader from "$lib/components/search-header.svelte";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";

    type LogKind = "user" | "system";

    const SERVICES = [
        "api-gateway",
        "registry",
        "volume-manager",
        "scheduler",
        "failover-controller",
        "sdn-controller",
        "object-storage",
        "agent",
    ];
    const LEVELS = ["DEBUG", "INFO", "WARN", "ERROR"];
    const CLASSIFICATIONS = ["security", "performance", "audit", "system", "network", "storage"];
    const REFRESH_INTERVAL_MS = 7000;
    const PAGE_LIMIT = 100;

    const initialNode = page.url.searchParams.get("node") ?? "";

    let kind = $state<LogKind>(initialNode ? "system" : "user");
    let entries = $state<LogEntry[]>([]);
    let total = $state(0);
    let hasMore = $state(false);
    let offset = $state(0);
    let loading = $state(true);
    let refreshing = $state(false);
    let error = $state<string | null>(null);
    let refreshTimer: ReturnType<typeof setInterval> | null = null;

    let nodes = $state<Node[]>([]);
    let users = $state<OrganizationUser[]>([]);

    let selectedService = $state("");
    let selectedLevel = $state("");
    let selectedClassification = $state("");
    let searchText = $state("");
    let fromTime = $state("");
    let toTime = $state("");
    let agentId = $state(initialNode);
    let actorId = $state("");
    let action = $state("");

    let selectedEntry = $state<LogEntry | null>(null);
    let detailOpen = $state(false);

    const nodeNames = $derived(new Map(nodes.map((node) => [node.id, node.name])));
    const userNames = $derived(new Map(users.map((user) => [user.id, user.username])));

    function currentFilter(): LogsFilter {
        return {
            kind,
            service: kind === "system" ? selectedService || undefined : undefined,
            level: kind === "system" ? selectedLevel || undefined : undefined,
            classification: kind === "system" ? selectedClassification || undefined : undefined,
            agent_id: kind === "system" ? agentId || undefined : undefined,
            actor_user_id: kind === "user" ? actorId || undefined : undefined,
            action: kind === "user" ? action || undefined : undefined,
            q: searchText || undefined,
            from: fromTime ? new Date(fromTime).toISOString() : undefined,
            to: toTime ? new Date(toTime).toISOString() : undefined,
            limit: PAGE_LIMIT,
            offset,
        };
    }

    async function load() {
        if (!auth.token) return;
        try {
            const response = await listLogs(auth.token, currentFilter());
            entries = response.entries;
            total = response.total;
            hasMore = response.has_more;
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
        const [nodeResult, userResult] = await Promise.allSettled([
            listNodes(auth.token),
            listUsers(auth.token),
        ]);
        if (nodeResult.status === "fulfilled") nodes = nodeResult.value;
        if (userResult.status === "fulfilled") users = userResult.value;
    }

    function refreshNow() {
        refreshing = true;
        load();
    }

    function switchKind(next: LogKind) {
        if (kind === next) return;
        kind = next;
        loading = true;
        entries = [];
    }

    function levelClass(level: string): string {
        switch (level) {
            case "ERROR": return "text-red-500";
            case "WARN": return "text-yellow-500";
            case "DEBUG": return "text-muted-foreground";
            default: return "text-foreground";
        }
    }

    function severityBars(level: string): number {
        switch (level) {
            case "ERROR": return 3;
            case "WARN": return 2;
            default: return 1;
        }
    }

    function severityBarClass(level: string): string {
        switch (level) {
            case "ERROR": return "bg-red-500";
            case "WARN": return "bg-yellow-500";
            default: return "bg-green-500";
        }
    }

    function formatDateTime(timestamp: string): string {
        return new Date(timestamp).toLocaleString();
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
        if (!entry.target_type) return "-";
        const id = entry.target_id && entry.target_id !== "-" ? ` ${entry.target_id.slice(0, 8)}` : "";
        return `${entry.target_type}${id}`;
    }

    function openDetail(entry: LogEntry) {
        selectedEntry = entry;
        detailOpen = true;
    }

    function nextPage() {
        if (!hasMore) return;
        offset += PAGE_LIMIT;
    }

    function prevPage() {
        offset = Math.max(0, offset - PAGE_LIMIT);
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
        selectedService;
        selectedLevel;
        selectedClassification;
        searchText;
        fromTime;
        toTime;
        agentId;
        actorId;
        action;
        offset;
        load();
    });

    $effect(() => {
        kind;
        selectedService;
        selectedLevel;
        selectedClassification;
        searchText;
        fromTime;
        toTime;
        agentId;
        actorId;
        action;
        offset = 0;
    });
</script>



<div class="flex h-svh min-h-0">
    <aside class="hidden shrink-0 flex-col overflow-y-auto border-r border-border pt-4 md:flex" style="width: 16rem;">
        <div class="p-4 flex flex-col gap-3">
            <h2 class="text-xs font-semibold uppercase text-muted-foreground">Filters</h2>

            {#if kind === "system"}
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="service-select">Service</label>
                    <select id="service-select" class="border rounded px-2 py-1.5 text-sm bg-background" bind:value={selectedService}>
                        <option value="">All services</option>
                        {#each SERVICES as service (service)}
                            <option value={service}>{service}</option>
                        {/each}
                    </select>
                </div>

                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="node-select">Node</label>
                    <select id="node-select" class="border rounded px-2 py-1.5 text-sm bg-background" bind:value={agentId}>
                        <option value="">All nodes</option>
                        {#each nodes as node (node.id)}
                            <option value={node.id}>{node.name}</option>
                        {/each}
                    </select>
                </div>

                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="level-select">Level</label>
                    <select id="level-select" class="border rounded px-2 py-1.5 text-sm bg-background" bind:value={selectedLevel}>
                        <option value="">All levels</option>
                        {#each LEVELS as level (level)}
                            <option value={level}>{level}</option>
                        {/each}
                    </select>
                </div>

                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="classification-select">Classification</label>
                    <select id="classification-select" class="border rounded px-2 py-1.5 text-sm bg-background" bind:value={selectedClassification}>
                        <option value="">All classifications</option>
                        {#each CLASSIFICATIONS as classification (classification)}
                            <option value={classification}>{classification}</option>
                        {/each}
                    </select>
                </div>
            {:else}
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="actor-select">User</label>
                    <select id="actor-select" class="border rounded px-2 py-1.5 text-sm bg-background" bind:value={actorId}>
                        <option value="">All users</option>
                        {#each users as user (user.id)}
                            <option value={user.id}>{user.username}</option>
                        {/each}
                    </select>
                </div>

                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="action-input">Action</label>
                    <Input id="action-input" placeholder="workloads.create" bind:value={action} class="font-mono text-xs" />
                </div>
            {/if}

            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="search-input">Search</label>
                <Input id="search-input" placeholder="message contains..." bind:value={searchText} />
            </div>

            <div class="grid grid-cols-2 gap-2">
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="from-input">From</label>
                    <input id="from-input" type="datetime-local" class="border rounded px-2 py-1.5 text-xs bg-background" bind:value={fromTime} />
                </div>
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="to-input">To</label>
                    <input id="to-input" type="datetime-local" class="border rounded px-2 py-1.5 text-xs bg-background" bind:value={toTime} />
                </div>
            </div>
        </div>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
        <SearchHeader />
        <div class="flex items-center gap-2 px-4 pb-3">
        <div class="flex rounded-md border p-0.5 text-sm">
            <button
                class="px-3 py-1 rounded {kind === 'user' ? 'bg-muted font-medium' : 'text-muted-foreground'}"
                onclick={() => switchKind("user")}
            >
                Activity
            </button>
            <button
                class="px-3 py-1 rounded {kind === 'system' ? 'bg-muted font-medium' : 'text-muted-foreground'}"
                onclick={() => switchKind("system")}
            >
                System
            </button>
        </div>
        <Button
            variant="outline"
            size="sm"
            class="ms-auto gap-1.5"
            onclick={refreshNow}
            disabled={refreshing}
        >
            <RefreshCwIcon class="size-3.5 {refreshing ? 'animate-spin' : ''}" />
            Refresh
        </Button>

        </div>
    <main class="flex min-h-0 flex-1 flex-col overflow-hidden border-t border-border">
        {#if error}
            <p class="p-4 text-sm text-destructive">{error}</p>
        {/if}

        <div class="flex-1 overflow-y-auto">
            <table class="w-full text-xs border-collapse">
                <thead class="sticky top-0 bg-background border-b z-10">
                    {#if kind === "system"}
                        <tr class="text-left text-[11px] text-muted-foreground uppercase">
                            <th class="px-2 py-1.5 font-semibold">Severity</th>
                            <th class="px-2 py-1.5 font-semibold">Service</th>
                            <th class="px-2 py-1.5 font-semibold">Node</th>
                            <th class="px-2 py-1.5 font-semibold">Level</th>
                            <th class="px-2 py-1.5 font-semibold">Classification</th>
                            <th class="px-2 py-1.5 font-semibold">Message</th>
                            <th class="px-2 py-1.5 font-semibold whitespace-nowrap">Last seen</th>
                        </tr>
                    {:else}
                        <tr class="text-left text-[11px] text-muted-foreground uppercase">
                            <th class="px-2 py-1.5 font-semibold whitespace-nowrap">Date / Time</th>
                            <th class="px-2 py-1.5 font-semibold">User</th>
                            <th class="px-2 py-1.5 font-semibold">Action</th>
                            <th class="px-2 py-1.5 font-semibold">Target</th>
                            <th class="px-2 py-1.5 font-semibold">Outcome</th>
                        </tr>
                    {/if}
                </thead>
                <tbody>
                    {#if loading}
                        <tr><td class="px-2 py-3 text-muted-foreground" colspan="7">Loading...</td></tr>
                    {:else if entries.length === 0}
                        <tr><td class="px-2 py-3 text-muted-foreground" colspan="7">No log entries.</td></tr>
                    {:else}
                        {#each entries as entry (entry.id)}
                            <tr
                                class="border-b hover:bg-muted/30 cursor-pointer"
                                onclick={() => openDetail(entry)}
                            >
                                {#if kind === "system"}
                                    <td class="px-2 py-1">
                                        <div class="flex gap-0.5" title={entry.level}>
                                            {#each [1, 2, 3] as bar (bar)}
                                                <span
                                                    class="h-2.5 w-1.5 rounded-sm {bar <= severityBars(entry.level) ? severityBarClass(entry.level) : 'bg-muted'}"
                                                ></span>
                                            {/each}
                                        </div>
                                    </td>
                                    <td class="px-2 py-1 font-medium whitespace-nowrap">{entry.service}</td>
                                    <td class="px-2 py-1 whitespace-nowrap">{nodeLabel(entry.agent_id)}</td>
                                    <td class="px-2 py-1 {levelClass(entry.level)} font-medium whitespace-nowrap">{entry.level}</td>
                                    <td class="px-2 py-1 whitespace-nowrap">{entry.classification}</td>
                                    <td class="px-2 py-1 truncate max-w-0">
                                        {entry.message}
                                        {#if entry.occurrences > 1}
                                            <span class="ms-1 rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">x{entry.occurrences}</span>
                                        {/if}
                                    </td>
                                    <td class="px-2 py-1 whitespace-nowrap text-muted-foreground">{formatDateTime(entry.last_seen_at)}</td>
                                {:else}
                                    <td class="px-2 py-1 whitespace-nowrap text-muted-foreground">{formatDateTime(entry.created_at)}</td>
                                    <td class="px-2 py-1 font-medium whitespace-nowrap">{actorLabel(entry.actor_user_id) === "-" ? entry.target_id ?? "-" : actorLabel(entry.actor_user_id)}</td>
                                    <td class="px-2 py-1 font-mono whitespace-nowrap">{entry.action ?? entry.message}</td>
                                    <td class="px-2 py-1 whitespace-nowrap">{targetLabel(entry)}</td>
                                    <td class="px-2 py-1 whitespace-nowrap {levelClass(entry.level)}">{entry.level === "WARN" ? "failed" : "ok"}</td>
                                {/if}
                            </tr>
                        {/each}
                    {/if}
                </tbody>
            </table>
        </div>

        <div class="flex items-center justify-between border-t px-4 py-2 text-xs text-muted-foreground">
            <span>{entries.length === 0 ? 0 : offset + 1}-{offset + entries.length} of {total} logs</span>
            <div class="flex gap-2">
                <button
                    class="px-2 py-1 rounded border disabled:opacity-40"
                    onclick={prevPage}
                    disabled={offset === 0}
                >
                    Prev
                </button>
                <button
                    class="px-2 py-1 rounded border disabled:opacity-40"
                    onclick={nextPage}
                    disabled={!hasMore}
                >
                    Next
                </button>
            </div>
        </div>
    </main>
    </div>
</div>

<Sheet.Root bind:open={detailOpen}>
    <Sheet.Content side="right" class="w-full sm:max-w-md overflow-y-auto">
        {#if selectedEntry}
            <Sheet.Header>
                <Sheet.Title>{formatDateTime(selectedEntry.created_at)}</Sheet.Title>
            </Sheet.Header>
            <div class="flex flex-col gap-3 px-4 pb-4 text-sm">
                <div class="flex justify-between border-b pb-2">
                    <span class="text-muted-foreground">Service</span>
                    <span class="font-medium">{selectedEntry.service}</span>
                </div>
                <div class="flex justify-between border-b pb-2">
                    <span class="text-muted-foreground">Level</span>
                    <span class="font-medium {levelClass(selectedEntry.level)}">{selectedEntry.level}</span>
                </div>
                <div class="flex justify-between border-b pb-2">
                    <span class="text-muted-foreground">Classification</span>
                    <span class="font-medium">{selectedEntry.classification}</span>
                </div>
                {#if selectedEntry.occurrences > 1}
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Occurrences</span>
                        <span class="font-medium">{selectedEntry.occurrences}</span>
                    </div>
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Last seen</span>
                        <span class="font-medium">{formatDateTime(selectedEntry.last_seen_at)}</span>
                    </div>
                {/if}
                {#if selectedEntry.actor_user_id}
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">User</span>
                        <span class="font-medium">{actorLabel(selectedEntry.actor_user_id)}</span>
                    </div>
                {/if}
                {#if selectedEntry.action}
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Action</span>
                        <span class="font-mono text-xs">{selectedEntry.action}</span>
                    </div>
                {/if}
                {#if selectedEntry.target_type}
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Target</span>
                        <span class="font-mono text-xs">{selectedEntry.target_type} {selectedEntry.target_id}</span>
                    </div>
                {/if}
                {#if selectedEntry.agent_id}
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Node</span>
                        <span class="font-medium">{nodeLabel(selectedEntry.agent_id)}</span>
                    </div>
                {/if}
                {#if selectedEntry.workload_id}
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Workload ID</span>
                        <span class="font-mono text-xs">{selectedEntry.workload_id}</span>
                    </div>
                {/if}
                {#each Object.entries(selectedEntry.fields ?? {}) as [key, value] (key)}
                    <div class="flex justify-between gap-4 border-b pb-2">
                        <span class="text-muted-foreground">{key}</span>
                        <span class="font-mono text-xs break-all text-right">{String(value)}</span>
                    </div>
                {/each}
                <div class="flex flex-col gap-1 pt-2">
                    <span class="text-xs font-semibold uppercase text-muted-foreground">Message</span>
                    <p class="whitespace-pre-wrap break-words">{selectedEntry.message}</p>
                </div>
            </div>
        {/if}
    </Sheet.Content>
</Sheet.Root>
