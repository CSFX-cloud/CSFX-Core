<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { getWorkloadVncUrl } from "$lib/api/resource-groups";

    let {
        token,
        workloadId,
        viewOnly = false,
    }: { token: string; workloadId: string; viewOnly?: boolean } = $props();

    let container = $state<HTMLDivElement | null>(null);
    let error = $state<string | null>(null);
    let rfb: import("@novnc/novnc").default | null = null;

    onMount(async () => {
        if (!container) return;
        try {
            const url = await getWorkloadVncUrl(token, workloadId);
            const { default: RFB } = await import("@novnc/novnc");
            rfb = new RFB(container, url);
            rfb.scaleViewport = true;
            rfb.viewOnly = viewOnly;
            rfb.addEventListener("disconnect", () => {
                error = "Console disconnected";
            });
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to connect to console";
        }
    });

    onDestroy(() => {
        rfb?.disconnect();
        rfb = null;
    });
</script>

<div class="flex flex-col h-full">
    {#if error}
        <p class="px-4 py-2 text-xs text-destructive shrink-0">{error}</p>
    {/if}
    <div class="flex-1 overflow-hidden bg-black" bind:this={container}></div>
</div>
