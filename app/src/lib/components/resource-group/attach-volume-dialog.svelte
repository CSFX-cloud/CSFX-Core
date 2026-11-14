<script lang="ts">
    import { createVolume, updateWorkload, type Volume, type Workload } from "$lib/api/resource-groups";
    import ModalDialog from "./modal-dialog.svelte";

    const NEW_VOLUME = "new";
    const DEFAULT_SIZE_GB = "10";

    type Props = { workload: Workload; rgId: string; available: Volume[]; onAttached: () => Promise<void> };

    let { workload, rgId, available, onAttached }: Props = $props();

    let dialog = $state<ModalDialog | null>(null);
    let choice = $state(NEW_VOLUME);
    let name = $state("");
    let sizeGb = $state(DEFAULT_SIZE_GB);
    let mountPath = $state("/data");
    let busy = $state(false);
    let error = $state<string | null>(null);

    const size = $derived(Number.parseInt(sizeGb, 10));
    const creating = $derived(choice === NEW_VOLUME);
    const usedPaths = $derived((workload.volume_mounts ?? []).map((mount) => mount.mount_path));
    const pathValid = $derived(mountPath.startsWith("/") && mountPath.length > 1 && !usedPaths.includes(mountPath));
    const volumeValid = $derived(!creating || (name.trim().length > 0 && Number.isInteger(size) && size > 0));
    const valid = $derived(pathValid && volumeValid);

    export function open() {
        dialog?.open();
    }

    async function resolveVolumeId(): Promise<string> {
        if (!creating) return choice;
        const created = await createVolume({ name: name.trim(), size_gb: size, resource_group_id: rgId });
        return created.id;
    }

    async function submit() {
        if (!valid) return;
        busy = true;
        error = null;
        try {
            const volumeId = await resolveVolumeId();
            const mounts = [...(workload.volume_mounts ?? []), { volume_id: volumeId, mount_path: mountPath }];
            await updateWorkload(workload.id, { volume_mounts: mounts });
            dialog?.close();
            await onAttached();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to add volume";
        } finally {
            busy = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Add volume" subtitle={workload.service_name ?? workload.name} onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="av-choice">Volume</label>
            <select class="cx-input" id="av-choice" bind:value={choice}>
                <option value={NEW_VOLUME}>Create new volume</option>
                {#each available as volume (volume.id)}
                    <option value={volume.id}>{volume.name} ({volume.size_gb} GB)</option>
                {/each}
            </select>
        </div>
        {#if creating}
            <div class="cx-field">
                <label class="cx-label" for="av-name">Name</label>
                <input class="cx-input" id="av-name" placeholder="postgres-data" autocomplete="off" bind:value={name} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="av-size">Size (GB)</label>
                <input class="cx-input" id="av-size" type="number" min="1" bind:value={sizeGb} />
            </div>
        {/if}
        <div class="cx-field">
            <label class="cx-label" for="av-path">Mount path</label>
            <input class="cx-input cx-mono" id="av-path" autocomplete="off" bind:value={mountPath} />
            <p class="cx-hint">The container restarts to mount the volume.</p>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={busy || !valid} aria-busy={busy}>{busy ? "Adding" : "Add"}</button>
        </div>
    </form>
</ModalDialog>
