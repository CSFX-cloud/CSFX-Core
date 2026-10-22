<script lang="ts">
    import { untrack } from "svelte";
    import type { UsageHistoryPoint } from "$lib/api/nodes";
    import type { Workload } from "$lib/api/resource-groups";
    import UsageChart from "$lib/components/dashboard/usage-chart.svelte";
    import { chartCeiling, USAGE_SERIES } from "$lib/utils/dashboard";
    import { formatBytes, formatPercent } from "$lib/utils/format";

    const SAMPLE_CAPACITY = 60;
    const PERCENT = 100;

    let { workload }: { workload: Workload } = $props();

    let samples = $state<UsageHistoryPoint[]>([]);

    const stopped = $derived(workload.status !== "running");
    const ceiling = $derived(chartCeiling(samples, ["cpu", "memory"]));
    const latest = $derived(samples.length ? samples[samples.length - 1] : null);
    const stats = $derived([
        { label: "CPU", value: workload.cpu_usage_percent !== null ? formatPercent(workload.cpu_usage_percent) : "—", note: `of ${workload.cpu_millicores}m requested` },
        { label: "Memory", value: workload.memory_usage_bytes !== null ? formatBytes(workload.memory_usage_bytes) : "—", note: `of ${formatBytes(workload.memory_bytes)} requested` },
        { label: "Network in", value: workload.network_rx_bytes !== null ? formatBytes(workload.network_rx_bytes) : "—", note: "Received" },
        { label: "Network out", value: workload.network_tx_bytes !== null ? formatBytes(workload.network_tx_bytes) : "—", note: "Transmitted" },
    ]);

    $effect(() => {
        const cpu = workload.cpu_usage_percent;
        const memoryBytes = workload.memory_usage_bytes;
        void workload.stats_updated_at;
        if (cpu === null || memoryBytes === null) return;
        const point: UsageHistoryPoint = {
            bucket: new Date().toISOString(),
            cpu,
            memory: workload.memory_bytes > 0 ? (memoryBytes / workload.memory_bytes) * PERCENT : 0,
            disk: 0,
        };
        samples = [...untrack(() => samples), point].slice(-SAMPLE_CAPACITY);
    });
</script>

<div style="display: grid; gap: 12px">
    <section class="db-stats" aria-label="Usage">
        {#each stats as stat (stat.label)}
            <div class="cx-stat">
                <p class="cx-stat__label">{stat.label}</p>
                <p class="db-stat-value">{stat.value}</p>
                <div class="cx-stat__foot">{stat.note}</div>
            </div>
        {/each}
    </section>
    <section class="cx-panel" aria-labelledby="ct-hist">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="ct-hist">Usage history</h2>
                <p class="cx-panel__sub">CPU and memory against what {workload.service_name ?? workload.name} requested</p>
            </div>
        </div>
        {#if stopped || samples.length < 2}
            <p class="cx-hint" style="padding: 32px 0; text-align: center">{stopped ? "No stats while the container is stopped." : "Collecting samples."}</p>
        {:else}
            <div class="db-legend">
                {#each USAGE_SERIES as series (series.key)}
                    <span><i style="background: {series.color}"></i>{series.label} {formatPercent(latest?.[series.key])}</span>
                {/each}
            </div>
            <UsageChart history={samples} range="1h" yMax={ceiling} label="CPU and memory of this container" />
        {/if}
    </section>
    {#if workload.restart_count > 0}
        <p class="cx-hint">
            Restarted {workload.restart_count} time{workload.restart_count === 1 ? "" : "s"}{workload.max_restarts !== null ? ` (max ${workload.max_restarts})` : ""}
        </p>
    {/if}
</div>
