<script lang="ts">
    import type { UsageHistoryPoint, UsageRange } from "$lib/api/nodes";
    import UsageChart from "$lib/components/dashboard/usage-chart.svelte";
    import type { UsageSample } from "$lib/utils/cluster-metrics";
    import { usageTone, USAGE_SERIES } from "$lib/utils/dashboard";

    const METERS = [...USAGE_SERIES, { key: "disk", label: "Disk" }] as const;

    const RANGES: { value: UsageRange; label: string }[] = [
        { value: "1h", label: "1H" },
        { value: "24h", label: "24H" },
        { value: "7d", label: "7D" },
    ];

    type Props = {
        history: UsageHistoryPoint[];
        latest: UsageSample | null;
        range: UsageRange;
        onrange: (range: UsageRange) => void;
    };

    let { history, latest, range, onrange }: Props = $props();
</script>

<section class="cx-panel" aria-labelledby="db-usage-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="db-usage-title">Live usage</h2>
            <p class="cx-panel__sub">Cluster-wide, all online servers</p>
        </div>
        <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Time range">
            {#each RANGES as entry (entry.value)}
                <button class="cx-tab" role="tab" type="button" aria-selected={entry.value === range} onclick={() => onrange(entry.value)}>
                    {entry.label}
                </button>
            {/each}
        </div>
    </div>
    <div class="db-legend">
        {#each USAGE_SERIES as series (series.key)}
            <span><i style="background: {series.color}"></i>{series.label}</span>
        {/each}
    </div>
    <UsageChart {history} {range} label="CPU and memory usage over the selected range" />
    <div class="db-meters">
        {#each METERS as meter (meter.key)}
            {@const value = latest ? latest[meter.key] : 0}
            <div class="db-meter">
                <span class="cx-stat__label" style="margin: 0">{meter.label}</span>
                <div class="cx-usage {usageTone(value) ? `cx-usage--${usageTone(value)}` : ''}">
                    <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {value}%"></div></div>
                    <span class="cx-usage__value">{latest ? `${value.toFixed(0)}%` : "-"}</span>
                </div>
            </div>
        {/each}
    </div>
</section>
