<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { listNodes, getClusterStats, type Node, type ClusterStats, type NodeMetrics } from "$lib/api/nodes";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import NodesInfoPanel from "$lib/components/nodes/nodes-info-panel.svelte";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
    import SearchIcon from "@lucide/svelte/icons/search";
    import BellIcon from "@lucide/svelte/icons/bell";
    import LayoutListIcon from "@lucide/svelte/icons/layout-list";
    import ArrowUpDownIcon from "@lucide/svelte/icons/arrow-up-down";
    import DownloadIcon from "@lucide/svelte/icons/download";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";

    const INFO_PANEL_WIDTH = "16rem";

    let nodes = $state<Node[]>([]);
    let statusFilter = $state<Set<string>>(new Set());

    type SortColumn = "hostname" | "status" | "cpu" | "memory" | "disk";
    const SORT_LABELS: Record<SortColumn, string> = {
        hostname: "Hostname",
        status: "Status",
        cpu: "CPU",
        memory: "Memory",
        disk: "Disk",
    };
    let sortColumn = $state<SortColumn>("hostname");
    let sortAscending = $state(true);

    function nodeMetrics(node: Node): NodeMetrics | null {
        return stats?.nodes.find((m) => m.agent_id === node.id) ?? null;
    }

    function metricRatio(bytesUsed: number | null, bytesTotal: number | null): number {
        if (bytesUsed == null || bytesTotal == null || bytesTotal === 0) return 0;
        return (bytesUsed / bytesTotal) * 100;
    }

    function sortValue(node: Node, column: SortColumn): number | string {
        const metrics = nodeMetrics(node);
        switch (column) {
            case "hostname": return node.hostname.toLowerCase();
            case "status": return node.status.toLowerCase();
            case "cpu": return metrics?.cpu_usage_percent ?? -1;
            case "memory": return metricRatio(metrics?.memory_used_bytes ?? null, metrics?.memory_total_bytes ?? null);
            case "disk": return metricRatio(metrics?.disk_used_bytes ?? null, metrics?.disk_total_bytes ?? null);
        }
    }

    function toggleSet(set: Set<string>, value: string): Set<string> {
        const next = new Set(set);
        if (next.has(value)) {
            next.delete(value);
        } else {
            next.add(value);
        }
        return next;
    }

    function toggleSort(column: SortColumn) {
        if (sortColumn === column) {
            sortAscending = !sortAscending;
        } else {
            sortColumn = column;
            sortAscending = true;
        }
    }

    const filteredNodes = $derived(
        nodes
            .filter((node) => {
                if (statusFilter.size > 0 && !statusFilter.has(node.status)) return false;
                return true;
            })
            .sort((a, b) => {
                const aValue = sortValue(a, sortColumn);
                const bValue = sortValue(b, sortColumn);
                const direction = sortAscending ? 1 : -1;
                if (aValue < bValue) return -1 * direction;
                if (aValue > bValue) return 1 * direction;
                return 0;
            })
    );
    let stats = $state<ClusterStats | null>(null);
    let loading = $state(true);
    let error = $state<string | null>(null);

    let pollInterval: ReturnType<typeof setInterval> | null = null;
    let refreshing = $state(false);
    let addNodePulsing = $state(false);

    async function fetchStats() {
        if (!auth.token) return;
        try {
            stats = await getClusterStats(auth.token);
        } catch {
            // non-fatal poll failure
        }
    }

    async function refresh() {
        if (!auth.token || refreshing) return;
        refreshing = true;
        try {
            [nodes] = await Promise.all([
                listNodes(auth.token),
                fetchStats(),
            ]);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to refresh";
        } finally {
            refreshing = false;
        }
    }

    let loadStarted = false;

    async function loadInitial() {
        try {
            [nodes] = await Promise.all([
                listNodes(auth.token!),
                fetchStats(),
            ]);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load nodes";
        } finally {
            loading = false;
        }
    }

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            loadInitial();
        }
    });

    onMount(() => {
        pollInterval = setInterval(fetchStats, 10_000);
    });

    onDestroy(() => {
        if (pollInterval) clearInterval(pollInterval);
    });

    function openNode(node: Node) {
        goto(`/nodes/${node.id}`);
    }

    function exportCsv() {
        const header = ["ID", "Hostname", "IP", "OS", "Architecture", "Status", "CPU %", "Memory %", "Disk %", "Heartbeat"];
        const rows = filteredNodes.map((node) => {
            const metrics = nodeMetrics(node);
            const cpu = metrics?.cpu_usage_percent ?? null;
            const memory = metricRatio(metrics?.memory_used_bytes ?? null, metrics?.memory_total_bytes ?? null);
            const disk = metricRatio(metrics?.disk_used_bytes ?? null, metrics?.disk_total_bytes ?? null);
            return [
                node.id,
                node.hostname,
                node.ip_address ?? "",
                `${node.os_type} ${node.os_version}`,
                node.architecture,
                node.status,
                cpu?.toFixed(1) ?? "",
                metrics ? memory.toFixed(1) : "",
                metrics ? disk.toFixed(1) : "",
                node.last_heartbeat ?? "",
            ];
        });
        const csv = [header, ...rows]
            .map((row) => row.map((cell) => `"${String(cell).replace(/"/g, '""')}"`).join(","))
            .join("\n");
        const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `nodes-${new Date().toISOString().slice(0, 10)}.csv`;
        link.click();
        URL.revokeObjectURL(url);
    }

    function barColor(percent: number): string {
        if (percent >= 90) return "text-destructive";
        if (percent >= 70) return "text-yellow-500";
        return "text-green-500";
    }
</script>

{#snippet metricBars(percent: number)}
    <div class="flex items-end gap-0.5 h-3.5" title="{percent.toFixed(0)}%">
        {#each [0.4, 0.7, 0.55, 1, 0.85] as heightRatio, i (i)}
            <span
                class="w-1 rounded-sm {barColor(percent)}"
                style="height: {heightRatio * 100}%; background-color: currentColor; opacity: {(i + 1) * 20 <= percent ? 1 : 0.25};"
            ></span>
        {/each}
    </div>
{/snippet}

<header class="flex h-16 shrink-0 items-center gap-3 px-4 border-b">
    <Sidebar.Trigger class="-ms-1" />
    <div class="relative w-full max-w-sm">
        <SearchIcon class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input placeholder="Search anything" class="pl-8 pr-14" />
        <kbd class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 rounded border border-border bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
            &#8984;K
        </kbd>
    </div>
    <div class="ms-auto flex items-center gap-1">
        <Button variant="ghost" size="icon-sm" aria-label="Notifications">
            <BellIcon class="size-4" />
        </Button>
    </div>
</header>

<div class="flex min-h-0 flex-1">
    <div
        class="hidden shrink-0 border-r border-border md:block"
        style="width: {INFO_PANEL_WIDTH};"
    >
        <NodesInfoPanel {stats} />
    </div>

    <div class="flex min-w-0 flex-1 flex-col gap-6 p-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-xl font-semibold tracking-tight">Nodes</h1>
            <p class="text-sm text-muted-foreground mt-0.5">Manage and monitor your cluster nodes</p>
        </div>
        <div class="flex items-center gap-2">
            <Button
                variant="outline"
                size="sm"
                onclick={refresh}
                disabled={refreshing}
                class={refreshing ? "opacity-60" : ""}
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    class={refreshing ? "animate-spin" : ""}
                >
                    <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/>
                    <path d="M21 3v5h-5"/>
                    <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/>
                    <path d="M8 16H3v5"/>
                </svg>
                Refresh
            </Button>
            <Button
                size="sm"
                class="relative overflow-hidden"
                onclick={() => { addNodePulsing = true; setTimeout(() => { addNodePulsing = false; }, 600); }}
            >
                {#if addNodePulsing}
                    <span class="absolute inset-0 rounded-md animate-ping bg-primary/30 pointer-events-none"></span>
                {/if}
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="12" y1="5" x2="12" y2="19"/>
                    <line x1="5" y1="12" x2="19" y2="12"/>
                </svg>
                Add Node
            </Button>
        </div>
    </div>

    <div class="border rounded-lg overflow-hidden">
        <div class="flex items-center justify-between px-4 py-3 border-b bg-muted/30">
            <div class="flex items-center gap-2">
                <LayoutListIcon class="size-4 text-muted-foreground" />
                <span class="text-sm font-semibold">Node Summary</span>
            </div>
            <div class="flex items-center gap-2">
                <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                            <Button {...props} variant="outline" size="sm">
                                Status
                                {#if statusFilter.size > 0}
                                    <span class="text-xs text-muted-foreground">({statusFilter.size})</span>
                                {/if}
                                <ChevronDownIcon class="size-3.5" />
                            </Button>
                        {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end">
                        {#each [...new Set(nodes.map((n) => n.status))] as status (status)}
                            <DropdownMenu.CheckboxItem
                                checked={statusFilter.has(status)}
                                onCheckedChange={() => (statusFilter = toggleSet(statusFilter, status))}
                            >
                                {status}
                            </DropdownMenu.CheckboxItem>
                        {/each}
                    </DropdownMenu.Content>
                </DropdownMenu.Root>

                <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                            <Button {...props} variant="outline" size="sm">
                                Sort: {SORT_LABELS[sortColumn]}
                                <ChevronDownIcon class="size-3.5" />
                            </Button>
                        {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end">
                        <DropdownMenu.RadioGroup value={sortColumn} onValueChange={(value) => (sortColumn = value as SortColumn)}>
                            {#each Object.entries(SORT_LABELS) as [value, label] (value)}
                                <DropdownMenu.RadioItem {value}>{label}</DropdownMenu.RadioItem>
                            {/each}
                        </DropdownMenu.RadioGroup>
                        <DropdownMenu.Separator />
                        <DropdownMenu.CheckboxItem
                            checked={!sortAscending}
                            onCheckedChange={(checked) => (sortAscending = !checked)}
                        >
                            Descending
                        </DropdownMenu.CheckboxItem>
                    </DropdownMenu.Content>
                </DropdownMenu.Root>

                <Button
                    variant="ghost"
                    size="icon-sm"
                    onclick={refresh}
                    disabled={refreshing}
                    aria-label="Refresh"
                >
                    <RefreshCwIcon class="size-4 {refreshing ? 'animate-spin' : ''}" />
                </Button>
                <Button
                    variant="ghost"
                    size="icon-sm"
                    onclick={exportCsv}
                    disabled={filteredNodes.length === 0}
                    aria-label="Export as CSV"
                >
                    <DownloadIcon class="size-4" />
                </Button>
            </div>
        </div>
        <table class="w-full text-sm">
            <thead class="bg-muted/50">
                <tr>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">ID</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">
                        <button class="flex items-center gap-1 hover:text-foreground" onclick={() => toggleSort("hostname")}>
                            Hostname
                            <ArrowUpDownIcon class="size-3 {sortColumn === 'hostname' ? 'text-foreground' : ''}" />
                        </button>
                    </th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">IP</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">OS</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Arch</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">
                        <button class="flex items-center gap-1 hover:text-foreground" onclick={() => toggleSort("status")}>
                            Status
                            <ArrowUpDownIcon class="size-3 {sortColumn === 'status' ? 'text-foreground' : ''}" />
                        </button>
                    </th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">
                        <button class="flex items-center gap-1 hover:text-foreground" onclick={() => toggleSort("cpu")}>
                            CPU
                            <ArrowUpDownIcon class="size-3 {sortColumn === 'cpu' ? 'text-foreground' : ''}" />
                        </button>
                    </th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">
                        <button class="flex items-center gap-1 hover:text-foreground" onclick={() => toggleSort("memory")}>
                            Memory
                            <ArrowUpDownIcon class="size-3 {sortColumn === 'memory' ? 'text-foreground' : ''}" />
                        </button>
                    </th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">
                        <button class="flex items-center gap-1 hover:text-foreground" onclick={() => toggleSort("disk")}>
                            Disk
                            <ArrowUpDownIcon class="size-3 {sortColumn === 'disk' ? 'text-foreground' : ''}" />
                        </button>
                    </th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Heartbeat</th>
                </tr>
            </thead>
            <tbody>
                {#if loading}
                    <tr>
                        <td colspan="10" class="px-4 py-8 text-center text-muted-foreground">Loading...</td>
                    </tr>
                {:else if error}
                    <tr>
                        <td colspan="10" class="px-4 py-8 text-center text-destructive">{error}</td>
                    </tr>
                {:else if nodes.length === 0}
                    <tr>
                        <td colspan="10" class="px-4 py-8 text-center text-muted-foreground">No nodes registered</td>
                    </tr>
                {:else if filteredNodes.length === 0}
                    <tr>
                        <td colspan="10" class="px-4 py-8 text-center text-muted-foreground">No nodes match the current filters</td>
                    </tr>
                {:else}
                    {#each filteredNodes as node (node.id)}
                        {@const metrics = nodeMetrics(node)}
                        {@const cpu = metrics?.cpu_usage_percent ?? 0}
                        {@const memory = metricRatio(metrics?.memory_used_bytes ?? null, metrics?.memory_total_bytes ?? null)}
                        {@const disk = metricRatio(metrics?.disk_used_bytes ?? null, metrics?.disk_total_bytes ?? null)}
                        <tr
                            class="border-t hover:bg-muted/30 transition-colors cursor-pointer"
                            onclick={() => openNode(node)}
                        >
                            <td class="px-4 py-3 font-mono text-xs text-muted-foreground">{node.id.slice(0, 8)}</td>
                            <td class="px-4 py-3 font-medium">{node.hostname}</td>
                            <td class="px-4 py-3 text-muted-foreground">{node.ip_address ?? "-"}</td>
                            <td class="px-4 py-3 text-muted-foreground">{node.os_type} {node.os_version}</td>
                            <td class="px-4 py-3 text-muted-foreground">{node.architecture}</td>
                            <td class="px-4 py-3">
                                <StatusBadge status={node.status} />
                            </td>
                            <td class="px-4 py-3">
                                {#if metrics}
                                    <div class="flex items-center gap-2">
                                        {@render metricBars(cpu)}
                                        <span class="text-xs text-muted-foreground tabular-nums">{cpu.toFixed(0)}%</span>
                                    </div>
                                {:else}
                                    <span class="text-xs text-muted-foreground">-</span>
                                {/if}
                            </td>
                            <td class="px-4 py-3">
                                {#if metrics}
                                    <div class="flex items-center gap-2">
                                        {@render metricBars(memory)}
                                        <span class="text-xs text-muted-foreground tabular-nums">{memory.toFixed(0)}%</span>
                                    </div>
                                {:else}
                                    <span class="text-xs text-muted-foreground">-</span>
                                {/if}
                            </td>
                            <td class="px-4 py-3">
                                {#if metrics}
                                    <div class="flex items-center gap-2">
                                        {@render metricBars(disk)}
                                        <span class="text-xs text-muted-foreground tabular-nums">{disk.toFixed(0)}%</span>
                                    </div>
                                {:else}
                                    <span class="text-xs text-muted-foreground">-</span>
                                {/if}
                            </td>
                            <td class="px-4 py-3 text-muted-foreground text-xs">
                                {node.last_heartbeat ? node.last_heartbeat.slice(0, 16) : "never"}
                            </td>
                        </tr>
                    {/each}
                {/if}
            </tbody>
        </table>
    </div>
    </div>
</div>
