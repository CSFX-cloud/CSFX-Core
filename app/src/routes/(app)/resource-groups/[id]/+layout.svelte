<script lang="ts">
    import { onDestroy } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteResourceGroup } from "$lib/api/resource-groups";
    import AppearanceDialog from "$lib/components/resource-group/appearance-dialog.svelte";
    import ResourceGroupSidebar from "$lib/components/resource-group/resource-group-sidebar.svelte";
    import { provideResourceGroupState, ResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const POLL_INTERVAL_MS = 2000;

    let { children } = $props();

    const rg = new ResourceGroupState($page.params.id ?? "");
    provideResourceGroupState(rg);

    let appearanceDialog = $state<AppearanceDialog | null>(null);
    let loadStarted = false;
    let pollInterval: ReturnType<typeof setInterval> | null = null;

    async function handleDeleteGroup() {
        if (!auth.token || !rg.group) return;
        try {
            await deleteResourceGroup(auth.token, rg.group.id);
            goto("/resource-groups");
        } catch (e) {
            rg.error = e instanceof Error ? e.message : "Failed to delete resource group";
        }
    }

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            rg.load();
        }
    });

    $effect(() => {
        if (rg.hasTransientStatus && !pollInterval) {
            pollInterval = setInterval(rg.load, POLL_INTERVAL_MS);
        } else if (!rg.hasTransientStatus && pollInterval) {
            clearInterval(pollInterval);
            pollInterval = null;
        }
    });

    onDestroy(() => {
        if (pollInterval) clearInterval(pollInterval);
    });
</script>

<AppearanceDialog bind:this={appearanceDialog} group={rg.group} onUpdated={(updated) => (rg.group = updated)} />

<div class="flex min-h-0 flex-1">
    <div class="hidden md:flex flex-col w-64 shrink-0 overflow-y-auto border-r border-border p-4">
        {#if rg.group}
            <ResourceGroupSidebar
                group={rg.group}
                resources={rg.summaries}
                onUpdated={(updated) => (rg.group = updated)}
                onEditIcon={() => appearanceDialog?.open(rg.group)}
                onDelete={handleDeleteGroup}
                onError={(message) => (rg.error = message)}
            />
        {/if}
    </div>
    <div class="flex min-w-0 flex-1 flex-col">
        {@render children()}
    </div>
</div>
