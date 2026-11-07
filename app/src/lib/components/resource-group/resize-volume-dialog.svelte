<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { resizeVolume, type Volume } from "$lib/api/resource-groups";
    import ModalDialog from "./modal-dialog.svelte";

    type Props = { onResized: () => Promise<void> };

    let { onResized }: Props = $props();

    let dialog = $state<ModalDialog | null>(null);
    let volume = $state<Volume | null>(null);
    let sizeGb = $state("");
    let busy = $state(false);
    let error = $state<string | null>(null);

    const size = $derived(Number.parseInt(sizeGb, 10));
    const valid = $derived(volume !== null && Number.isInteger(size) && size > volume.size_gb);

    export function open(target: Volume) {
        volume = target;
        sizeGb = String(target.size_gb * 2);
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !volume || !valid) return;
        busy = true;
        error = null;
        try {
            await resizeVolume(auth.token, volume.id, size);
            dialog?.close();
            await onResized();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to resize volume";
        } finally {
            busy = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Grow volume" subtitle={volume?.name} onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="rv-size">New size (GB)</label>
            <input class="cx-input" id="rv-size" type="number" min={(volume?.size_gb ?? 0) + 1} bind:value={sizeGb} />
            <p class="cx-hint">Currently {volume?.size_gb} GB. Volumes can only grow. The container restarts to use the new space.</p>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={busy || !valid} aria-busy={busy}>{busy ? "Growing" : "Grow"}</button>
        </div>
    </form>
</ModalDialog>
