<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createVolume } from "$lib/api/resource-groups";
    import ModalDialog from "./modal-dialog.svelte";

    const DEFAULT_SIZE_GB = "10";

    let { rgId, onCreated }: { rgId: string; onCreated: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let sizeGb = $state(DEFAULT_SIZE_GB);
    let creating = $state(false);
    let error = $state<string | null>(null);

    const size = $derived(Number.parseInt(sizeGb, 10));
    const valid = $derived(name.trim().length > 0 && Number.isInteger(size) && size > 0);

    export function open() {
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !valid) return;
        creating = true;
        error = null;
        try {
            await createVolume(auth.token, { name: name.trim(), size_gb: size, resource_group_id: rgId });
            dialog?.close();
            name = "";
            sizeGb = DEFAULT_SIZE_GB;
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create volume";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Create volume" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="v-name">Name</label>
            <input class="cx-input" id="v-name" placeholder="postgres-data" autocomplete="off" bind:value={name} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="v-size">Size (GB)</label>
            <input class="cx-input" id="v-size" type="number" min="1" placeholder={DEFAULT_SIZE_GB} bind:value={sizeGb} />
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !valid} aria-busy={creating}>
                {creating ? "Creating" : "Create"}
            </button>
        </div>
    </form>
</ModalDialog>
