<script lang="ts">
    import { onDestroy } from "svelte";
    import { page } from "$app/stores";
    import { auth } from "$lib/auth/store.svelte";
    import { provideResourceGroupState, ResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const POLL_INTERVAL_MS = 2000;

    let { children } = $props();

    const rg = new ResourceGroupState($page.params.id ?? "");
    provideResourceGroupState(rg);

    let loadStarted = false;
    let pollInterval: ReturnType<typeof setInterval> | null = null;

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

{@render children()}
