<script lang="ts">
    import type { Workload } from "$lib/api/resource-groups";
    import { fmtBytes } from "$lib/utils/format";

    let { workload }: { workload: Workload } = $props();

    const stats = $derived([
        {
            label: "CPU Usage",
            value: workload.cpu_usage_percent !== null ? `${workload.cpu_usage_percent.toFixed(1)}%` : "-",
            note: `${workload.cpu_millicores}m requested`,
        },
        {
            label: "Memory Usage",
            value: workload.memory_usage_bytes !== null ? fmtBytes(workload.memory_usage_bytes) : "-",
            note: `${fmtBytes(workload.memory_bytes)} requested`,
        },
        {
            label: "Network RX",
            value: workload.network_rx_bytes !== null ? fmtBytes(workload.network_rx_bytes) : "-",
            note: null,
        },
        {
            label: "Network TX",
            value: workload.network_tx_bytes !== null ? fmtBytes(workload.network_tx_bytes) : "-",
            note: null,
        },
    ]);
</script>

<div class="h-full overflow-y-auto p-6">
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
        {#each stats as stat}
            <div class="border rounded-lg p-4">
                <p class="text-xs text-muted-foreground">{stat.label}</p>
                <p class="text-2xl font-semibold mt-1">{stat.value}</p>
                {#if stat.note}
                    <p class="text-xs text-muted-foreground mt-0.5">{stat.note}</p>
                {/if}
            </div>
        {/each}
    </div>
    <p class="text-xs text-muted-foreground mt-4">
        {workload.stats_updated_at ? `Last updated ${new Date(workload.stats_updated_at).toLocaleTimeString()}` : "No stats reported yet."}
    </p>
    {#if workload.restart_count > 0}
        <p class="text-xs text-muted-foreground mt-1">
            Restarted {workload.restart_count} time{workload.restart_count === 1 ? "" : "s"}{workload.max_restarts !== null ? ` (max ${workload.max_restarts})` : ""}
        </p>
    {/if}
</div>
