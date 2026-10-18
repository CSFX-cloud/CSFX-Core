<script lang="ts">
    import type { FacetCount } from "$lib/api/logs";
    import { toggleValue } from "$lib/utils/logs";

    let {
        title,
        facets,
        selected = $bindable(),
    }: { title: string; facets: FacetCount[]; selected: string[] } = $props();
</script>

{#if facets.length > 0}
    <div class="flex flex-col gap-1">
        <h3 class="text-sm font-semibold">{title}</h3>
        {#each facets as facet (facet.value)}
            <label class="flex items-center gap-2 py-0.5 text-sm">
                <input
                    type="checkbox"
                    class="size-4 rounded border-input accent-primary"
                    checked={selected.includes(facet.value)}
                    onchange={() => (selected = toggleValue(selected, facet.value))}
                />
                <span class="truncate">{facet.value}</span>
                <span class="text-muted-foreground">({facet.count})</span>
            </label>
        {/each}
    </div>
{/if}
