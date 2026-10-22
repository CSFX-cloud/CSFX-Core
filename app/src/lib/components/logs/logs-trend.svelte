<script lang="ts">
    import type { LogsTrendPoint } from "$lib/api/logs";

    const MIN_BAR_PERCENT = 4;
    const EDGE_FRACTION = 0.12;

    let { points }: { points: LogsTrendPoint[] } = $props();

    let hoverIndex = $state<number | null>(null);

    const peak = $derived(Math.max(1, ...points.map((point) => point.count)));
    const hovered = $derived(hoverIndex !== null ? (points[hoverIndex] ?? null) : null);
    const position = $derived(points.length > 1 && hoverIndex !== null ? hoverIndex / (points.length - 1) : 0);
    const shift = $derived(position < EDGE_FRACTION ? "0%" : position > 1 - EDGE_FRACTION ? "-100%" : "-50%");

    function track(event: PointerEvent) {
        if (points.length === 0) return;
        const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const fraction = Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width));
        hoverIndex = Math.min(points.length - 1, Math.floor(fraction * points.length));
    }
</script>

<div class="lg-trend" role="img" aria-label="Log events over the selected time range" onpointermove={track} onpointerleave={() => (hoverIndex = null)}>
    {#each points as point, index (point.start)}
        <span class:active={index === hoverIndex} style="height: {Math.max(MIN_BAR_PERCENT, (point.count / peak) * 100)}%"></span>
    {/each}
    {#if hovered}
        <div class="lg-trend__tip" style="left: {position * 100}%; transform: translateX({shift})">
            <b>{hovered.count.toLocaleString()}</b>
            <span>{new Date(hovered.start).toLocaleString()}</span>
        </div>
    {/if}
</div>
