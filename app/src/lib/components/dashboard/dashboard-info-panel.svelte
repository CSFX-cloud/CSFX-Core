<script lang="ts">
    import { uptimeColor, type UsageSample } from "$lib/utils/cluster-metrics";
    import UsageChart from "./usage-chart.svelte";

    let {
        uptimePercent,
        usage,
        usageCapacity,
    }: {
        uptimePercent: number | null;
        usage: UsageSample[];
        usageCapacity: number;
    } = $props();
</script>

<div class="flex h-full flex-col gap-5 overflow-y-auto px-4 pb-4">
    <span class="text-label uppercase text-muted-foreground tracking-[0.02em]">Dashboard</span>

    <div class="flex items-baseline justify-between gap-2">
        <span class="text-sm text-muted-foreground">Uptime (7d)</span>
        <span class="text-lg font-semibold tabular-nums" style="color: {uptimeColor(uptimePercent)}">
            {uptimePercent !== null ? `${uptimePercent.toFixed(2)}%` : "-"}
        </span>
    </div>

    <div class="flex flex-col gap-2">
        <span class="text-sm text-muted-foreground">Live usage</span>
        <UsageChart samples={usage} capacity={usageCapacity} />
    </div>
</div>
