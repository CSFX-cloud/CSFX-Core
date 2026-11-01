<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createVault, type Vault } from "$lib/api/vaults";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId, onCreated }: { rgId: string; onCreated: (vault: Vault) => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
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
            const vault = await createVault(auth.token, rgId, name.trim());
            dialog?.close();
            name = "";
            await onCreated(vault);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create vault";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Create key vault" subtitle="Secrets stay encrypted. Resources read them at start." onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="kv-name">Name</label>
            <input class="cx-input" id="kv-name" placeholder="kv-prod" autocomplete="off" bind:value={name} />
            <p class="cx-hint">Lowercase letters, digits and hyphens.</p>
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
