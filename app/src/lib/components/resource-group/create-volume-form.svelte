<script lang="ts">
    import { createVolume } from "$lib/api/resource-groups";

    const DEFAULT_SIZE_GB = "10";

    let { rgId, onCreated, oncancel }: { rgId: string; onCreated: () => Promise<void>; oncancel: () => void } = $props();

    let name = $state("");
    let sizeGb = $state(DEFAULT_SIZE_GB);
    let creating = $state(false);
    let error = $state<string | null>(null);

    const size = $derived(Number.parseInt(sizeGb, 10));
    const valid = $derived(name.trim().length > 0 && Number.isInteger(size) && size > 0);

    async function submit() {
        if (!valid) return;
        creating = true;
        error = null;
        try {
            await createVolume({ name: name.trim(), size_gb: size, resource_group_id: rgId });
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create volume";
        } finally {
            creating = false;
        }
    }
</script>

<form
    class="md__fields md__fields--two"
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
    {#if error}<p class="cx-hint cx-hint--error md__span">{error}</p>{/if}
    <div class="md__foot md__span">
        <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={oncancel}>Cancel</button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !valid} aria-busy={creating}>
            {creating ? "Creating" : "Create"}
        </button>
    </div>
</form>
