<script lang="ts" module>
    import type { DeletePreview } from "$lib/utils/delete-preview";

    export interface DeleteTarget {
        title: string;
        name: string;
        requireStop: boolean;
        load: () => Promise<DeletePreview>;
        run: (preview: DeletePreview, stopRunning: boolean) => Promise<void>;
    }
</script>

<script lang="ts">
    import { TriangleAlert } from "@lucide/svelte";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    let dialog = $state<ModalDialog | null>(null);
    let target = $state.raw<DeleteTarget | null>(null);
    let preview = $state<DeletePreview | null>(null);
    let loadError = $state<string | null>(null);
    let error = $state<string | null>(null);
    let typed = $state("");
    let stopRunning = $state(false);
    let busy = $state(false);

    const stopMissing = $derived(Boolean(target?.requireStop && preview?.running.length && !stopRunning));
    const canDelete = $derived(Boolean(target && preview && !busy && !stopMissing && typed === target.name));

    function reset() {
        preview = null;
        loadError = null;
        error = null;
        typed = "";
        stopRunning = false;
        busy = false;
    }

    export async function open(next: DeleteTarget) {
        reset();
        target = next;
        dialog?.open();
        try {
            const loaded = await next.load();
            if (target === next) preview = loaded;
        } catch (e) {
            if (target === next) loadError = e instanceof Error ? e.message : "Failed to load the resources that would be deleted";
        }
    }

    async function confirm() {
        if (!target || !preview || !canDelete) return;
        busy = true;
        error = null;
        try {
            await target.run(preview, stopRunning);
            dialog?.close();
        } catch (e) {
            error = e instanceof Error ? e.message : "Deletion failed";
        } finally {
            busy = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title={target?.title ?? "Delete"} size="xl" onclose={reset}>
    <form
        class="md__fields dd"
        onsubmit={(event) => {
            event.preventDefault();
            confirm();
        }}
    >
        {#if loadError}
            <p class="cx-hint cx-hint--error">{loadError}. Deletion cannot be started.</p>
        {:else if !preview}
            <div class="db-skeleton-list" role="status" aria-label="Loading affected resources">
                {#each { length: 3 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
            </div>
        {:else}
            <p class="db-sub" style="margin: 0; font-size: 12px">
                {preview.entries.length} {preview.entries.length === 1 ? "resource" : "resources"} will be deleted
                ({preview.vmCount} {preview.vmCount === 1 ? "VM" : "VMs"}, {preview.volumeCount} {preview.volumeCount === 1 ? "volume" : "volumes"}). This cannot be undone.
            </p>
            <div class="dd-list">
                <table class="cx-table">
                    <thead><tr><th>Type</th><th>Name</th><th>Status</th></tr></thead>
                    <tbody>
                        {#each preview.entries as entry, index (index)}
                            <tr>
                                <td>{entry.type}</td>
                                <td class="cx-mono">{entry.name}</td>
                                <td><span class={badgeClass(entry.status)}>{statusLabel(entry.status)}</span></td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
            {#if preview.dependencies.length > 0}
                <div>
                    <p class="cx-label">Affected dependencies</p>
                    <ul class="cx-hint" style="margin: 4px 0 0; padding-left: 18px; font-size: 12px">
                        {#each preview.dependencies as dependency (dependency)}<li>{dependency}</li>{/each}
                    </ul>
                </div>
            {/if}
            {#if preview.running.length > 0}
                <div class="st-note st-split">
                    <span class="dd-warn">
                        <TriangleAlert class="cx-i" aria-hidden="true" />
                        <span>{preview.running.length} running {preview.running.length === 1 ? "workload" : "workloads"} will lose all state.
                        {#if target?.requireStop}Shutdown is required before deletion.{/if}</span>
                    </span>
                    <Switch checked={stopRunning} label="Shut down first" onchange={(value) => (stopRunning = value)} />
                </div>
            {/if}
            <div class="cx-field">
                <label class="cx-label dd-label" for="delete-confirm-name">Type <span class="cx-mono">{target?.name}</span> to confirm</label>
                <input class="cx-input cx-mono" id="delete-confirm-name" type="text" autocomplete="off" bind:value={typed} />
            </div>
        {/if}
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm nd-danger" type="submit" disabled={!canDelete} aria-busy={busy}>
                {busy ? "Deleting" : "Delete"}
            </button>
        </div>
    </form>
</ModalDialog>

<style>
    .dd-list {
        max-height: 320px;
        overflow: auto;
    }
    .dd :global(.cx-table) {
        font-size: 12px;
    }
    .dd :global(.cx-table td) {
        padding: 6px 12px 6px 0;
    }
    .dd :global(.cx-table th) {
        padding-bottom: 6px;
        font-size: 12px;
    }
    .dd-warn {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .dd-label {
        display: block;
    }
</style>
