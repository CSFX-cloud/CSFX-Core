<script lang="ts">
    import { Pin, Plus } from "@lucide/svelte";
    import type { ResourceGroup } from "$lib/api/resource-groups";
    import GroupTile from "$lib/components/resource-group/group-tile.svelte";

    let { groups, oncreate }: { groups: ResourceGroup[]; oncreate: () => void } = $props();
</script>

<div class="rg-cards">
    {#each groups as group (group.id)}
        <a class="cx-card rg-card" href="/resource-groups/{group.id}">
            <div class="rg-card__top">
                <GroupTile {group} size="lg" />
            </div>
            <div>
                <h3 style="display: flex; align-items: center; gap: 6px">{group.name}{#if group.pinned}<Pin class="cx-i rg-pin" aria-label="Pinned" />{/if}</h3>
                <p>{group.description ?? "No description"}</p>
            </div>
            <div class="rg-card__meta"><span>{group.internal_cidr}</span><span>{group.created_at.slice(0, 10)}</span></div>
        </a>
    {/each}
    <button class="rg-new" type="button" onclick={oncreate}>
        <Plus class="cx-i" aria-hidden="true" />
        New resource group
    </button>
</div>
