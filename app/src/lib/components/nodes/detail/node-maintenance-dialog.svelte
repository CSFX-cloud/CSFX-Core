<script lang="ts">
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    const DEFAULT_MINUTES = "60";

    type Props = { busy: boolean; error: string | null; onsubmit: (minutes: number) => void };

    let { busy, error, onsubmit }: Props = $props();

    let dialog = $state<ModalDialog | null>(null);
    let minutes = $state(DEFAULT_MINUTES);

    const parsed = $derived(Number.parseInt(minutes, 10));
    const valid = $derived(Number.isInteger(parsed) && parsed >= 1);

    export function open() {
        dialog?.open();
    }

    export function close() {
        dialog?.close();
    }
</script>

<ModalDialog bind:this={dialog} title="Start maintenance" subtitle="Hardware alerts for this node are suppressed until maintenance ends.">
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            if (valid) onsubmit(parsed);
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="maintenance-minutes">Duration (minutes)</label>
            <input class="cx-input" id="maintenance-minutes" type="number" min="1" aria-invalid={!valid} bind:value={minutes} />
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={close}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={!valid || busy} aria-busy={busy}>
                {busy ? "Starting" : "Start maintenance"}
            </button>
        </div>
    </form>
</ModalDialog>
