<script lang="ts">
    import type { Workload } from "$lib/api/resource-groups";
    import GaugeCard from "$lib/components/gauge-card.svelte";
    import { fmtBytes } from "$lib/utils/format";

    let { workload }: { workload: Workload } = $props();

    const cpuPercent = $derived(workload.cpu_usage_percent ?? 0);
    const memoryPercent = $derived(
        workload.memory_usage_bytes !== null ? (workload.memory_usage_bytes / workload.memory_bytes) * 100 : 0,
    );
    const lastUpdated = $derived(
        workload.stats_updated_at
            ? `Last updated ${new Date(workload.stats_updated_at).toLocaleTimeString()}`
            : "No stats reported yet.",
    );
</script>

<div class="px-6 py-4 flex flex-col gap-3">
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <GaugeCard value={cpuPercent} label="CPU" detail="{workload.cpu_millicores / 1000} vCPU" />
        <GaugeCard
            value={memoryPercent}
            label="Memory"
            detail="{workload.memory_usage_bytes !== null ? `${fmtBytes(workload.memory_usage_bytes)} / ` : ''}{fmtBytes(workload.memory_bytes)}"
        />
        <GaugeCard value={0} label="Disk" detail="{fmtBytes(workload.disk_bytes)} allocated" />
        <div class="p-3 flex flex-col items-center justify-center gap-1 text-center">
            <span class="text-xs font-medium">Network</span>
            <span class="text-xs text-blue-500">RX {workload.network_rx_bytes !== null ? fmtBytes(workload.network_rx_bytes) : "-"}</span>
            <span class="text-xs text-purple-500">TX {workload.network_tx_bytes !== null ? fmtBytes(workload.network_tx_bytes) : "-"}</span>
        </div>
    </div>
    <p class="text-xs text-muted-foreground">{lastUpdated}</p>
    {#if workload.restart_count > 0}
        <p class="text-xs text-muted-foreground">
            Restarted {workload.restart_count} time{workload.restart_count === 1 ? "" : "s"}
        </p>
    {/if}
</div>
