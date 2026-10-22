<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import type { Workload } from "$lib/api/resource-groups";
    import { serviceIcon } from "$lib/components/shell/service-icons";
    import VncConsole from "$lib/components/vnc-console.svelte";

    let { workload, onOpen }: { workload: Workload; onOpen: () => void } = $props();

    const live = $derived(workload.status === "running" && !!auth.token);
    const glyph = serviceIcon("virtual-machines").svg;
</script>

<button class="vm-screen" type="button" aria-label="Open console" title={live ? "Open console" : "Console preview is available while the VM is running"} onclick={onOpen}>
    {#if live && auth.token}
        {#key workload.id}
            <div style="position: absolute; inset: 0; pointer-events: none">
                <VncConsole token={auth.token} workloadId={workload.id} viewOnly />
            </div>
        {/key}
        <span class="vm-screen__live"><i></i>LIVE</span>
    {:else}
        <span class="vm-screen__off">
            <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html glyph}</svg>
            Powered off
        </span>
    {/if}
</button>
