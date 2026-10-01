<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createBucket } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
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
        if (!auth.token || !name) return;
        creating = true;
        error = null;
        try {
            await createBucket(auth.token, { name, resource_group_id: rgId, exposure });
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

<ModalDialog bind:this={dialog} title="Create Bucket" onclose={() => (error = null)}>
    <div class="grid grid-cols-1 gap-3">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="b-name">Name</label>
            <input id="b-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="assets" bind:value={name} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="b-exposure">Exposure</label>
            <select id="b-exposure" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={exposure}>
                <option value="internal">Internal (resource group only)</option>
                <option value="external">External (public endpoint)</option>
            </select>
        </div>
    </div>
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex gap-2 justify-end">
        <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
        <Button size="sm" onclick={submit} disabled={creating || !name}>
            {creating ? "Creating..." : "Create"}
        </Button>
    </div>
</ModalDialog>
