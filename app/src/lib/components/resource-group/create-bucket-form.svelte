<script lang="ts">
    import { createBucket } from "$lib/api/resource-groups";
    import { Select } from "$lib/components/ui/select";

    let { rgId, onCreated, oncancel }: { rgId: string; onCreated: () => Promise<void>; oncancel: () => void } = $props();

    let name = $state("");
    let exposure = $state<"internal" | "external">("internal");
    let creating = $state(false);
    let error = $state<string | null>(null);

    async function submit() {
        if (!name.trim()) return;
        creating = true;
        error = null;
        try {
            await createBucket({ name: name.trim(), resource_group_id: rgId, exposure });
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create bucket";
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
        <label class="cx-label" for="b-name">Name</label>
        <input class="cx-input" id="b-name" placeholder="assets" autocomplete="off" bind:value={name} />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="b-exposure">Exposure</label>
        <Select
            id="b-exposure"
            bind:value={exposure}
            options={[
                { value: "internal", label: "Internal (resource group only)" },
                { value: "external", label: "External (public endpoint)" },
            ]}
        />
    </div>
    {#if error}<p class="cx-hint cx-hint--error md__span">{error}</p>{/if}
    <div class="md__foot md__span">
        <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={oncancel}>Cancel</button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !name.trim()} aria-busy={creating}>
            {creating ? "Creating" : "Create"}
        </button>
    </div>
</form>
