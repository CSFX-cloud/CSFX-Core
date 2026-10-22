<script lang="ts">
    import type { UsageHistoryPoint, UsageRange } from "$lib/api/nodes";
    import { formatBucket, pointPosition, seriesPaths, USAGE_SERIES } from "$lib/utils/dashboard";

    const GRID_PATH = "M0 1H600M0 50.5H600M0 100H600M0 149.5H600M0 199H600";
    const AXIS_FRACTIONS = [1, 0.75, 0.5, 0.25, 0];
    const TOOLTIP_FLIP_FRACTION = 0.6;

    type Props = { history: UsageHistoryPoint[]; range: UsageRange; yMax?: number; label: string };

    let { history, range, yMax = 100, label }: Props = $props();

    const uid = $props.id();
    let hoverIndex = $state<number | null>(null);

    const axis = $derived(AXIS_FRACTIONS.map((fraction) => (fraction === 0 ? "0" : `${+(yMax * fraction).toFixed(1)}%`)));
    const paths = $derived(USAGE_SERIES.map((series) => ({ ...series, ...seriesPaths(history, series.key, yMax) })));
    const hovered = $derived(hoverIndex !== null ? (history[hoverIndex] ?? null) : null);
    const markers = $derived.by(() => {
        const index = hoverIndex;
        if (index === null || hovered === null) return [];
        return USAGE_SERIES.map((series) => ({
            ...series,
            value: hovered[series.key],
            ...pointPosition(index, history.length, hovered[series.key], yMax),
        }));
    });

    function trackPointer(event: PointerEvent) {
        if (history.length < 2) return;
        const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const fraction = Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width));
        hoverIndex = Math.round(fraction * (history.length - 1));
    }
</script>

<div class="db-chart">
    <div class="db-chart__y" aria-hidden="true">
        {#each axis as tick (tick)}<span>{tick}</span>{/each}
    </div>
    <div class="db-chart__plot" role="presentation" onpointermove={trackPointer} onpointerleave={() => (hoverIndex = null)}>
        <svg viewBox="0 0 600 200" preserveAspectRatio="none" role="img" aria-label={label}>
            <defs>
                {#each USAGE_SERIES as series (series.key)}
                    <linearGradient id="{uid}-{series.key}" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" style="stop-color: {series.color}; stop-opacity: {series.fillOpacity}"></stop>
                        <stop offset="1" style="stop-color: {series.color}; stop-opacity: 0"></stop>
                    </linearGradient>
                {/each}
            </defs>
            <path class="db-gridline" d={GRID_PATH}></path>
            {#each paths as path (path.key)}
                {#if path.line}
                    <path d={path.area} fill="url(#{uid}-{path.key})"></path>
                    <path class="db-line" d={path.line} style="stroke: {path.color}"></path>
                {/if}
            {/each}
        </svg>
        {#if hovered && markers.length}
            {@const left = markers[0].left}
            <div class="db-hover__line" style="left: {left}%"></div>
            {#each markers as marker (marker.key)}
                <span class="db-hover__dot" style="left: {marker.left}%; top: {marker.top}%; background: {marker.color}"></span>
            {/each}
            <div class="db-tooltip" style="left: {left}%; transform: translateX({left / 100 > TOOLTIP_FLIP_FRACTION ? 'calc(-100% - 12px)' : '12px'})">
                <time datetime={hovered.bucket}>{formatBucket(hovered.bucket, range)}</time>
                {#each markers as marker (marker.key)}
                    <span class="db-tooltip__row">
                        <span><i style="background: {marker.color}"></i>{marker.label}</span>
                        <b>{marker.value.toFixed(1)}%</b>
                    </span>
                {/each}
            </div>
        {/if}
    </div>
</div>
