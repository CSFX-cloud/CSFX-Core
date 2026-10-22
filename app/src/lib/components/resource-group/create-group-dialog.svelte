<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createResourceGroup, suggestCidr, type ResourceGroup } from "$lib/api/resource-groups";
    import IconPicker from "$lib/components/icon-picker.svelte";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    const DEFAULT_CIDR = "10.100.0.0/24";
    const DEFAULT_ICON = "mdi:cube-outline";
    const DEFAULT_COLOR = "#6366f1";

    let { oncreated }: { oncreated: (group: ResourceGroup) => void } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let cidr = $state(DEFAULT_CIDR);
    let description = $state("");
    let icon = $state(DEFAULT_ICON);
    let color = $state(DEFAULT_COLOR);
    let creating = $state(false);
    let error = $state<string | null>(null);

    const valid = $derived(name.trim().length > 0 && cidr.trim().length > 0);

    function reset() {
        name = "";
        cidr = DEFAULT_CIDR;
        description = "";
        icon = DEFAULT_ICON;
        color = DEFAULT_COLOR;
        error = null;
    }

    export async function open() {
        if (auth.token) cidr = await suggestCidr(auth.token).catch(() => DEFAULT_CIDR);
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !valid) return;
        creating = true;
        error = null;
        try {
            const created = await createResourceGroup(auth.token, {
                name: name.trim(),
                description: description.trim() || undefined,
                internal_cidr: cidr.trim(),
                icon,
                color,
            });
            oncreated(created);
            dialog?.close();
            reset();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create resource group";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="New resource group" size="md" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <p class="db-sub" style="margin: 0; font-size: 13px">Gets its own internal network and stays isolated from other groups.</p>
        <div class="cx-field">
            <label class="cx-label" for="rg-name">Name</label>
            <input class="cx-input" id="rg-name" type="text" placeholder="production" autocomplete="off" bind:value={name} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="rg-cidr">Internal network (CIDR)</label>
            <input class="cx-input cx-mono" id="rg-cidr" type="text" placeholder={DEFAULT_CIDR} autocomplete="off" aria-describedby="rg-cidr-hint" bind:value={cidr} />
            <p class="cx-hint" id="rg-cidr-hint">Must not overlap with another group.</p>
        </div>
        <div class="cx-field">
            <label class="cx-label" for="rg-desc">Description <span style="color: var(--ink-muted)">Optional</span></label>
            <input class="cx-input" id="rg-desc" type="text" placeholder="What runs in this group" autocomplete="off" bind:value={description} />
        </div>
        <div class="md__sep"></div>
        <IconPicker bind:icon bind:color />
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !valid} aria-busy={creating}>
                {creating ? "Creating" : "Create group"}
            </button>
        </div>
    </form>
</ModalDialog>
