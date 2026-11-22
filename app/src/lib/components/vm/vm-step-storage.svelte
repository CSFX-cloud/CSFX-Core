<script lang="ts">
    import type { Volume } from "$lib/api/resource-groups";
    import type { VmDraft } from "./vm-draft.svelte";

    let { draft, volumes }: { draft: VmDraft; volumes: Volume[] } = $props();

    const available = $derived(volumes.filter((volume) => volume.attached_to_workload === null || draft.volumeIds.includes(volume.id)));
</script>

<div class="md__fields md__fields--two">
    <div class="cx-field md__span">
        <label class="cx-label" for="vm-disk-gb">Boot disk (GB)</label>
        <input id="vm-disk-gb" type="number" min="1" step="1" class="cx-input" bind:value={draft.diskGb} />
        <p class="cx-hint">The boot disk is created empty and the ISO installs the system onto it.</p>
    </div>
    <div class="cx-field md__span">
        <span class="cx-label">Block storage</span>
        {#if available.length === 0}
            <p class="cx-hint">No free volumes in this resource group. Create a volume first to attach it as an extra disk.</p>
        {:else}
            {#each available as volume (volume.id)}
                <label class="cx-check">
                    <input type="checkbox" checked={draft.volumeIds.includes(volume.id)} onchange={() => draft.toggleVolume(volume.id)} />
                    {volume.name} <span class="cx-hint">{volume.size_gb} GB, {volume.pool}</span>
                </label>
            {/each}
            <p class="cx-hint">Attached volumes show up in the guest as additional VirtIO block devices (vdb, vdc, and so on).</p>
        {/if}
    </div>
</div>
