<script lang="ts">
    import type { UsageHistoryPoint, UsageRange } from "$lib/api/nodes";
    import type { UsageSample } from "$lib/utils/cluster-metrics";
    import { formatBucket, pointPosition, seriesPaths, usageTone } from "$lib/utils/dashboard";

    const SERIES = [
        { key: "cpu", label: "CPU", color: "#2dd4bf", fillOpacity: 0.25 },
        { key: "memory", label: "Memory", color: "#3b82f6", fillOpacity: 0.35 },
    ] as const;
    const METERS = [...SERIES, { key: "disk", label: "Disk" }] as const;
    const GRID_PATH = "M0 1H600M0 50.5H600M0 100H600M0 149.5H600M0 199H600";

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

    const TOOLTIP_FLIP_FRACTION = 0.6;

    let hoverIndex = $state<number | null>(null);

    const paths = $derived(SERIES.map((series) => ({ ...series, ...seriesPaths(history, series.key) })));
    const hovered = $derived(hoverIndex !== null ? (history[hoverIndex] ?? null) : null);
    const markers = $derived(
        hovered && hoverIndex !== null
            ? SERIES.map((series) => ({ ...series, value: hovered[series.key], ...pointPosition(hoverIndex!, history.length, hovered[series.key]) }))
            : [],
    );

    function trackPointer(event: PointerEvent) {
        if (history.length < 2) return;
        const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const fraction = Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width));
        hoverIndex = Math.round(fraction * (history.length - 1));
    }
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
        {#each SERIES as series (series.key)}
            <span><i style="background: {series.color}"></i>{series.label}</span>
        {/each}
    </div>
    <div class="db-chart">
        <div class="db-chart__y" aria-hidden="true"><span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0</span></div>
        <div class="db-chart__plot" role="presentation" onpointermove={trackPointer} onpointerleave={() => (hoverIndex = null)}>
        <svg viewBox="0 0 600 200" preserveAspectRatio="none" role="img" aria-label="CPU and memory usage over the selected range">
            <defs>
                {#each SERIES as series (series.key)}
                    <linearGradient id="db-gradient-{series.key}" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" style="stop-color: {series.color}; stop-opacity: {series.fillOpacity}"></stop>
                        <stop offset="1" style="stop-color: {series.color}; stop-opacity: 0"></stop>
                    </linearGradient>
                {/each}
            </defs>
            <path class="db-gridline" d={GRID_PATH}></path>
            {#each paths as path (path.key)}
                {#if path.line}
                    <path d={path.area} fill="url(#db-gradient-{path.key})"></path>
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
