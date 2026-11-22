<script lang="ts">
    import type { Volume } from "$lib/api/resource-groups";
    import CreateVolumeForm from "$lib/components/resource-group/create-volume-form.svelte";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    type Mode = "existing" | "new";
    type Props = { rgId: string; free: Volume[]; onPick: (volumeId: string) => void; onVolumeCreated: () => Promise<void> };

    let { rgId, free, onPick, onVolumeCreated }: Props = $props();

    let dialog = $state<ModalDialog | null>(null);
    let mode = $state<Mode>("existing");
    let selectedId = $state("");

    export function open() {
        mode = free.length > 0 ? "existing" : "new";
        selectedId = free[0]?.id ?? "";
        dialog?.open();
    }

    function addExisting() {
        if (selectedId === "") return;
        onPick(selectedId);
        dialog?.close();
    }

    async function created(volume: Volume) {
        await onVolumeCreated();
        onPick(volume.id);
        dialog?.close();
    }
</script>

<ModalDialog bind:this={dialog} title="Add block storage" subtitle="Attached as an additional disk">
    <div class="md__fields">
        <div class="cx-tabs cx-tabs--contained cx-tabs--sm vm-storage-tabs" role="tablist" aria-label="Block storage source">
            <button class="cx-tab" type="button" role="tab" aria-selected={mode === "existing"} disabled={free.length === 0} onclick={() => (mode = "existing")}>Use existing</button>
            <button class="cx-tab" type="button" role="tab" aria-selected={mode === "new"} onclick={() => (mode = "new")}>Create new</button>
        </div>
        {#if mode === "existing"}
            <div class="cx-field">
                {#each free as volume (volume.id)}
                    <label class="cx-check">
                        <input type="radio" name="vm-storage-volume" value={volume.id} bind:group={selectedId} />
                        {volume.name} <span class="cx-hint">{volume.size_gb} GB, {volume.pool}</span>
                    </label>
                {/each}
            </div>
            <div class="md__foot">
                <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={selectedId === ""} onclick={addExisting}>Add</button>
            </div>
        {:else}
            <CreateVolumeForm {rgId} onCreated={created} oncancel={() => dialog?.close()} />
        {/if}
    </div>
</ModalDialog>
