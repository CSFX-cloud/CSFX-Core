<script lang="ts">
    import { CalendarDays } from "@lucide/svelte";
    import type { LogsFacets } from "$lib/api/logs";
    import type { Node } from "$lib/api/nodes";
    import { LEVELS, levelMeta, RANGES, toggleValue, type RangeKey } from "$lib/utils/logs";
    import FacetList from "./facet-list.svelte";

    type LogKind = "user" | "system";

    type Props = {
        kind: LogKind;
        range: RangeKey;
        levels: string[];
        classifications: string[];
        services: string[];
        targetTypes: string[];
        actions: string[];
        agentId: string;
        fromTime: string;
        toTime: string;
        facets: LogsFacets | null;
        nodes: Node[];
        onclear: () => void;
    };

    let {
        kind = $bindable(),
        range = $bindable(),
        levels = $bindable(),
        classifications = $bindable(),
        services = $bindable(),
        targetTypes = $bindable(),
        actions = $bindable(),
        agentId = $bindable(),
        fromTime = $bindable(),
        toTime = $bindable(),
        facets,
        nodes,
        onclear,
    }: Props = $props();

    const kindCounts = $derived(new Map((facets?.kinds ?? []).map((facet) => [facet.value, facet.count])));
    const streams: { value: LogKind; label: string }[] = [
        { value: "system", label: "General" },
        { value: "user", label: "Audit" },
    ];
</script>

<aside class="lg-side" aria-label="Filters">
    <div>
        <h2>Time range</h2>
        <div style="display: flex; gap: 6px; align-items: center">
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Time range" style="flex: 1">
                {#each RANGES as entry (entry.key)}
                    <button class="cx-tab" role="tab" type="button" aria-selected={range === entry.key} style="flex: 1; padding: 0 8px" onclick={() => (range = entry.key)}>
                        {entry.label}
                    </button>
                {/each}
            </div>
            <button
                class="cx-icon-btn cx-icon-btn--sm lg-calendar"
                type="button"
                aria-label="Custom range"
                aria-pressed={range === "custom"}
                onclick={() => (range = "custom")}
            >
                <CalendarDays class="cx-i" aria-hidden="true" />
            </button>
        </div>
        {#if range === "custom"}
            <div style="display: grid; gap: 8px; margin-top: 10px">
                <input type="datetime-local" class="cx-input lg-input" aria-label="From" bind:value={fromTime} />
                <input type="datetime-local" class="cx-input lg-input" aria-label="To" bind:value={toTime} />
            </div>
        {/if}
    </div>

    <div>
        <h2>Severity</h2>
        <div class="lg-sev" role="group" aria-label="Severity">
            {#each LEVELS as level (level)}
                {@const meta = levelMeta(level)}
                <button type="button" aria-pressed={levels.includes(level)} onclick={() => (levels = toggleValue(levels, level))}>
                    <i style="background: {meta.dot}"></i>{meta.label}
                </button>
            {/each}
        </div>
    </div>

    <div>
        <h2>Stream</h2>
        <div class="cx-tabs cx-tabs--contained cx-tabs--sm cx-tabs--block" role="tablist" aria-label="Stream">
            {#each streams as stream (stream.value)}
                <button class="cx-tab" role="tab" type="button" aria-selected={kind === stream.value} style="padding: 0 8px" onclick={() => (kind = stream.value)}>
                    {stream.label} {(kindCounts.get(stream.value) ?? 0).toLocaleString()}
                </button>
            {/each}
        </div>
    </div>

    {#if kind === "system"}
        <FacetList title="Category" facets={facets?.classifications ?? []} bind:selected={classifications} />
        <FacetList title="Service" facets={facets?.services ?? []} bind:selected={services} />
        <div>
            <h2><label for="lg-node">Node</label></h2>
            <select class="cx-input lg-select" id="lg-node" bind:value={agentId}>
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

    <div class="lg-links">
        <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" style="justify-content: flex-start; padding: 0" onclick={onclear}>Clear filters</button>
    </div>
</aside>
