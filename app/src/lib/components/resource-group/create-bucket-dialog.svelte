<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createBucket } from "$lib/api/resource-groups";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId, onCreated }: { rgId: string; onCreated: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let exposure = $state<"internal" | "external">("internal");
    let creating = $state(false);
    let error = $state<string | null>(null);

    export function open() {
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !name.trim()) return;
        creating = true;
        error = null;
        try {
            await createBucket(auth.token, { name: name.trim(), resource_group_id: rgId, exposure });
            dialog?.close();
            name = "";
            exposure = "internal";
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create bucket";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Create bucket" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="b-name">Name</label>
            <input class="cx-input" id="b-name" placeholder="assets" autocomplete="off" bind:value={name} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="b-exposure">Exposure</label>
            <select class="cx-input" id="b-exposure" bind:value={exposure}>
                <option value="internal">Internal (resource group only)</option>
                <option value="external">External (public endpoint)</option>
            </select>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !name.trim()} aria-busy={creating}>
                {creating ? "Creating" : "Create"}
            </button>
        </div>
    </form>
</ModalDialog>
