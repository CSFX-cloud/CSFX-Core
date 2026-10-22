<script lang="ts">
    import type { HistoryPoint } from "$lib/components/nodes/detail/node-live.svelte";
    import { formatRate } from "$lib/utils/format";
    import { trafficCeiling, trafficPaths } from "$lib/utils/node-detail";

    const GRID_PATH = "M0 1H600M0 70H600M0 139H600";
    const RX_COLOR = "#2dd4bf";
    const TX_COLOR = "#3b82f6";

    let { points, height = 140 }: { points: HistoryPoint[]; height?: number } = $props();

    const latest = $derived(points.length ? points[points.length - 1] : null);
    const ceiling = $derived(trafficCeiling(points));
    const paths = $derived(trafficPaths(points, ceiling));
</script>

<div class="db-legend">
    <span><i style="background: {RX_COLOR}"></i>RX {formatRate(latest?.rxBps ?? 0)}</span>
    <span><i style="background: {TX_COLOR}"></i>TX {formatRate(latest?.txBps ?? 0)}</span>
</div>
<div class="db-chart db-chart--wide" style="height: {height}px">
    <div class="db-chart__y" aria-hidden="true">
        <span>{formatRate(ceiling)}</span><span>{formatRate(ceiling / 2)}</span><span>0</span>
    </div>
    <svg viewBox="0 0 600 140" preserveAspectRatio="none" role="img" aria-label="Network traffic over the last minutes">
        <path class="db-gridline" d={GRID_PATH}></path>
        {#if paths.rx}
            <path class="db-line" d={paths.rx} style="stroke: {RX_COLOR}"></path>
            <path class="db-line" d={paths.tx} style="stroke: {TX_COLOR}"></path>
        {/if}
    </svg>
</div>
