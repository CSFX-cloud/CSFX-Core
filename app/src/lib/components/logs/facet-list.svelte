<script lang="ts">
    import type { FacetCount } from "$lib/api/logs";
    import { toggleValue } from "$lib/utils/logs";

    let { title, facets, selected = $bindable() }: { title: string; facets: FacetCount[]; selected: string[] } = $props();
</script>

{#if facets.length > 0}
    <div>
        <h2>{title}</h2>
        <div class="lg-checks">
            {#each facets as facet (facet.value)}
                <label class="cx-check">
                    <input type="checkbox" checked={selected.includes(facet.value)} onchange={() => (selected = toggleValue(selected, facet.value))} />
                    <span class="lg-facet">{facet.value}</span>
                    <em>{facet.count.toLocaleString()}</em>
                </label>
            {/each}
        </div>
    </div>
{/if}
