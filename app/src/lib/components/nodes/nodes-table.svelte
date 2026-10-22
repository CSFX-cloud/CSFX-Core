<script lang="ts">
    import { ArrowUpDown, ChevronRight, Download, Search, Server, ServerOff } from "@lucide/svelte";
    import type { Node, NodeMetrics } from "$lib/api/nodes";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { nodeTone, usageTone } from "$lib/utils/dashboard";
    import {
        diskPercent,
        formatHeartbeat,
        isInMaintenance,
        maintenanceLabel,
        matchesQuery,
        matchesTab,
        memoryPercent,
        nodesCsv,
        sortValue,
        type SortColumn,
        type StatusTab,
    } from "$lib/utils/nodes";

    type Props = { nodes: Node[]; metrics: NodeMetrics[]; loading: boolean; error: string | null };

    const TABS: { value: StatusTab; label: string }[] = [
        { value: "all", label: "All" },
        { value: "online", label: "Online" },
        { value: "offline", label: "Offline" },
    ];
    const COLUMNS: { column: SortColumn; label: string }[] = [
        { column: "hostname", label: "Hostname" },
        { column: "status", label: "Status" },
    ];
    const USAGE_COLUMNS: { column: SortColumn; label: string }[] = [
        { column: "cpu", label: "CPU" },
        { column: "memory", label: "Memory" },
        { column: "disk", label: "Disk" },
    ];
    const SKELETON_ROWS = 3;

    let { nodes, metrics, loading, error }: Props = $props();

    let query = $state("");
    let tab = $state<StatusTab>("all");
    let sortColumn = $state<SortColumn>("hostname");
    let sortAscending = $state(true);

    const metricsById = $derived(new Map(metrics.map((entry) => [entry.agent_id, entry])));
    const visible = $derived(
        nodes
            .filter((node) => matchesTab(node, tab) && matchesQuery(node, query))
            .sort((a, b) => compare(a, b)),
    );

    function count(value: StatusTab): number {
        return nodes.filter((node) => matchesTab(node, value)).length;
    }

    function compare(a: Node, b: Node): number {
        const left = sortValue(a, metricsById.get(a.id), sortColumn);
        const right = sortValue(b, metricsById.get(b.id), sortColumn);
        const direction = sortAscending ? 1 : -1;
        if (left === right) return 0;
        return (left < right ? -1 : 1) * direction;
    }

    function toggleSort(column: SortColumn) {
        sortAscending = sortColumn === column ? !sortAscending : true;
        sortColumn = column;
    }

    function exportCsv() {
        const blob = new Blob([nodesCsv(visible, metricsById)], { type: "text/csv;charset=utf-8;" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `nodes-${new Date().toISOString().slice(0, 10)}.csv`;
        link.click();
        URL.revokeObjectURL(url);
    }

    function label(value: string): string {
        return value.charAt(0).toUpperCase() + value.slice(1).toLowerCase();
    }
</script>

{#snippet sortHeader(column: SortColumn, text: string)}
    <th aria-sort={sortColumn === column ? (sortAscending ? "ascending" : "descending") : "none"}>
        <button class="nl-sort" type="button" onclick={() => toggleSort(column)}>
            {text}
            <ArrowUpDown class="cx-i" aria-hidden="true" style={sortColumn === column ? "color: var(--ink)" : ""} />
        </button>
    </th>
{/snippet}

{#snippet usage(percent: number | null)}
    {#if percent === null}
        <span class="cx-num">&mdash;</span>
    {:else}
        <div class="cx-usage {usageTone(percent) ? `cx-usage--${usageTone(percent)}` : ''}">
            <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {percent}%"></div></div>
            <span class="cx-usage__value">{percent.toFixed(0)}%</span>
        </div>
    {/if}
{/snippet}

<section class="cx-panel" aria-labelledby="nl-title">
    <h2 class="cx-panel__title db-sr-only" id="nl-title">All nodes</h2>
    <div class="nl-toolbar">
        <div class="nl-toolbar__left">
            <div class="cx-input-wrap nl-search">
                <Search class="cx-i" aria-hidden="true" />
                <input class="cx-input cx-input--icon" type="search" placeholder="Filter by hostname or IP" aria-label="Filter nodes" bind:value={query} />
            </div>
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Status">
                {#each TABS as entry (entry.value)}
                    <button class="cx-tab" role="tab" type="button" aria-selected={tab === entry.value} onclick={() => (tab = entry.value)}>
                        {entry.label} {count(entry.value)}
                    </button>
                {/each}
            </div>
        </div>
        <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Export as CSV" disabled={visible.length === 0} onclick={exportCsv}>
            <Download class="cx-i" aria-hidden="true" />
        </button>
    </div>

    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if nodes.length === 0}
        <EmptyState icon={ServerOff} label="No nodes registered" />
    {:else if visible.length === 0}
        <EmptyState icon={Search} label="No nodes match the current filters" />
    {:else}
        <div class="db-scroll">
            <table class="cx-table">
                <thead>
                    <tr>
                        {#each COLUMNS as entry (entry.column)}{@render sortHeader(entry.column, entry.label)}{/each}
                        <th>IP address</th>
                        <th>OS</th>
                        {#each USAGE_COLUMNS as entry (entry.column)}{@render sortHeader(entry.column, entry.label)}{/each}
                        <th>Last heartbeat</th>
                        <th><span class="db-sr-only">Open</span></th>
                    </tr>
                </thead>
                <tbody>
                    {#each visible as node (node.id)}
                        {@const entry = metricsById.get(node.id)}
                        {@const online = node.status.toLowerCase() === "online"}
                        <tr class="nl-row">
                            <td>
                                <div class="nl-cell">
                                    <span class="{online ? 'cx-tile-glow' : 'cx-tile-plain'} nl-tile" aria-hidden="true"><Server class="cx-i" /></span>
                                    <div class="nl-host">
                                        <a href="/nodes/{node.id}">{node.hostname}</a>
                                        <span>{node.id.slice(0, 8)} &middot; {node.role}</span>
                                    </div>
                                </div>
                            </td>
                            <td>
                                {#if isInMaintenance(node)}
                                    <span class="cx-badge cx-badge--warning" title={maintenanceLabel(node.maintenance_until ?? "")}>Maintenance</span>
                                {:else}
                                    <span class="cx-badge cx-badge--{nodeTone(node.status)}">{label(node.status)}</span>
                                {/if}
                            </td>
                            <td class="cx-num">{node.ip_address ?? "—"}</td>
                            <td class="cx-soft">{node.os_type} {node.os_version} &middot; {node.architecture}</td>
                            <td>{@render usage(entry?.cpu_usage_percent ?? null)}</td>
                            <td>{@render usage(memoryPercent(entry))}</td>
                            <td>{@render usage(diskPercent(entry))}</td>
                            <td class="cx-num" style={online || !node.last_heartbeat ? "" : "color: var(--danger)"}>{formatHeartbeat(node.last_heartbeat)}</td>
                            <td>
                                <a class="nl-go" href="/nodes/{node.id}" aria-label="Open {node.hostname} ({node.ip_address ?? node.id.slice(0, 8)})">
                                    <ChevronRight class="cx-i" aria-hidden="true" />
                                </a>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</section>
