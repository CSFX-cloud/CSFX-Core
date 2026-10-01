<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createVolume } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId, onCreated }: { rgId: string; onCreated: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let sizeGb = $state("10");
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
            await createVolume(auth.token, { name, size_gb: parseInt(sizeGb), resource_group_id: rgId });
            dialog?.close();
            name = "";
            sizeGb = "10";
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create volume";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Create Volume" onclose={() => (error = null)}>
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="v-name">Name</label>
            <input id="v-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="postgres-data" bind:value={name} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="v-size">Size (GB)</label>
            <input id="v-size" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="10" bind:value={sizeGb} />
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
