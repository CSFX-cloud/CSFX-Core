<script lang="ts">
    import { ChevronRight, Pin } from "@lucide/svelte";
    import type { ResourceGroup } from "$lib/api/resource-groups";
    import GroupTile from "$lib/components/resource-group/group-tile.svelte";

    let { groups }: { groups: ResourceGroup[] } = $props();
</script>

<section class="cx-panel" aria-label="All resource groups">
    <div class="db-scroll">
        <table class="cx-table">
            <thead>
                <tr><th>Name</th><th>Network</th><th>Description</th><th>Created</th><th><span class="db-sr-only">Open</span></th></tr>
            </thead>
            <tbody>
                {#each groups as group (group.id)}
                    <tr class="nl-row">
                        <td>
                            <a class="rg-name" href="/resource-groups/{group.id}">
                                <GroupTile {group} />
                                <b>{group.name}{#if group.pinned}<Pin class="cx-i rg-pin" aria-label="Pinned" />{/if}</b>
                            </a>
                        </td>
                        <td class="cx-num">{group.internal_cidr}</td>
                        <td class="cx-soft">{group.description ?? "—"}</td>
                        <td class="cx-num">{group.created_at.slice(0, 10)}</td>
                        <td><span class="nl-go"><ChevronRight class="cx-i" aria-hidden="true" /></span></td>
                    </tr>
                {/each}
            </tbody>
        </table>
    </div>
</section>
