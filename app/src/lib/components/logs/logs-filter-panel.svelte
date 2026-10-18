<script lang="ts">
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import SearchIcon from "@lucide/svelte/icons/search";
    import CalendarIcon from "@lucide/svelte/icons/calendar-days";
    import type { LogsFacets } from "$lib/api/logs";
    import type { Node } from "$lib/api/nodes";
    import { LEVELS, RANGES, toggleValue, type RangeKey } from "$lib/utils/logs";
    import FacetList from "./facet-list.svelte";
    import SeverityBars from "./severity-bars.svelte";

    type LogKind = "user" | "system";

    let {
        kind = $bindable(),
        range = $bindable(),
        levels = $bindable(),
        classifications = $bindable(),
        services = $bindable(),
        targetTypes = $bindable(),
        actions = $bindable(),
        agentId = $bindable(),
        searchText = $bindable(),
        fromTime = $bindable(),
        toTime = $bindable(),
        facets,
        nodes,
        refreshing,
        onrefresh,
        onclear,
        onexport,
    }: {
        kind: LogKind;
        range: RangeKey;
        levels: string[];
        classifications: string[];
        services: string[];
        targetTypes: string[];
        actions: string[];
        agentId: string;
        searchText: string;
        fromTime: string;
        toTime: string;
        facets: LogsFacets | null;
        nodes: Node[];
        refreshing: boolean;
        onrefresh: () => void;
        onclear: () => void;
        onexport: () => void;
    } = $props();

    const kindCounts = $derived(new Map((facets?.kinds ?? []).map((facet) => [facet.value, facet.count])));
</script>

<div class="flex flex-col gap-4 p-4">
    <div class="flex items-center gap-2">
        <div class="relative flex-1">
            <SearchIcon class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <input
                class="h-9 w-full rounded-md border border-input bg-background pl-8 pr-2 text-sm"
                placeholder="Search"
                bind:value={searchText}
            />
        </div>
        <button
            type="button"
            class="flex size-9 items-center justify-center rounded-md border border-input text-muted-foreground hover:text-foreground"
            title="Refresh"
            aria-label="Refresh"
            onclick={onrefresh}
        >
            <RefreshCwIcon class="size-4 {refreshing ? 'animate-spin' : ''}" />
        </button>
    </div>

    <div class="flex items-center rounded-md border border-input text-sm">
        {#each RANGES as entry (entry.key)}
            <button
                type="button"
                class="flex-1 py-1.5 {range === entry.key ? 'font-semibold text-primary' : 'text-muted-foreground'}"
                onclick={() => (range = entry.key)}
            >
                {entry.label}
            </button>
        {/each}
        <button
            type="button"
            class="px-3 py-1.5 {range === 'custom' ? 'text-primary' : 'text-muted-foreground'}"
            title="Custom range"
            aria-label="Custom range"
            onclick={() => (range = "custom")}
        >
            <CalendarIcon class="size-4" />
        </button>
    </div>

    {#if range === "custom"}
        <div class="grid grid-cols-2 gap-2">
            <input type="datetime-local" class="rounded border bg-background px-2 py-1.5 text-xs" bind:value={fromTime} />
            <input type="datetime-local" class="rounded border bg-background px-2 py-1.5 text-xs" bind:value={toTime} />
        </div>
    {/if}

    <div class="grid grid-cols-4 gap-1.5">
        {#each LEVELS as level (level)}
            <button
                type="button"
                class="flex items-center justify-center rounded-md border py-2 {levels.includes(level) ? 'border-primary bg-muted' : 'border-input'}"
                title={level}
                onclick={() => (levels = toggleValue(levels, level))}
            >
                <SeverityBars {level} />
            </button>
        {/each}
    </div>

    <div class="flex items-center gap-4 text-sm">
        <label class="flex items-center gap-2">
            <input type="radio" class="accent-primary" checked={kind === "system"} onchange={() => (kind = "system")} />
            General ({kindCounts.get("system") ?? 0})
        </label>
        <label class="flex items-center gap-2">
            <input type="radio" class="accent-primary" checked={kind === "user"} onchange={() => (kind = "user")} />
            Audit ({kindCounts.get("user") ?? 0})
        </label>
    </div>

    {#if kind === "system"}
        <FacetList title="Category" facets={facets?.classifications ?? []} bind:selected={classifications} />
        <FacetList title="Service" facets={facets?.services ?? []} bind:selected={services} />
        <div class="flex flex-col gap-1">
            <label class="text-sm font-semibold" for="node-filter">Node</label>
            <select id="node-filter" class="h-9 rounded-md border border-input bg-background px-2 text-sm" bind:value={agentId}>
                <option value="">All nodes</option>
                {#each nodes as node (node.id)}
                    <option value={node.id}>{node.name}</option>
                {/each}
            </select>
        </div>
    {:else}
        <FacetList title="Resource" facets={facets?.target_types ?? []} bind:selected={targetTypes} />
        <FacetList title="Action" facets={facets?.actions ?? []} bind:selected={actions} />
    {/if}

    <button type="button" class="self-start text-sm text-primary hover:underline" onclick={onclear}>
        Clear filters
    </button>
    <button type="button" class="self-start text-sm text-primary hover:underline" onclick={onexport}>
        Export as CEF
    </button>
</div>
