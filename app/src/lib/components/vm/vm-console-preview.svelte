<script lang="ts">
    import MonitorIcon from "@lucide/svelte/icons/monitor";
    import { auth } from "$lib/auth/store.svelte";
    import type { Workload } from "$lib/api/resource-groups";
    import VncConsole from "$lib/components/vnc-console.svelte";

    let { workload, onOpen }: { workload: Workload; onOpen: () => void } = $props();

    const live = $derived(workload.status === "running" && !!auth.token);
</script>

<button
    class="relative block w-72 shrink-0 overflow-hidden rounded-lg border bg-muted/40 aspect-video cursor-pointer hover:border-foreground/40 transition-colors"
    onclick={onOpen}
    aria-label="Open console"
    title={live ? "Open console" : "Console preview is available while the vm is running"}
>
    {#if live && auth.token}
        {#key workload.id}
            <div class="absolute inset-0 pointer-events-none">
                <VncConsole token={auth.token} workloadId={workload.id} viewOnly />
            </div>
        {/key}
    {:else}
        <div class="absolute inset-0 flex flex-col items-center justify-center gap-2.5 animate-pulse">
            <MonitorIcon class="size-8 text-muted-foreground" />
            <div class="h-2 w-24 rounded-full bg-foreground/10"></div>
            <div class="h-2 w-14 rounded-full bg-foreground/10"></div>
        </div>
    {/if}
</button>
