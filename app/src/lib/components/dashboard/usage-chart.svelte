<script lang="ts">
    import type { UsageSample } from "$lib/utils/cluster-metrics";

    const WIDTH = 200;
    const HEIGHT = 80;
    const MAX_PERCENT = 100;

    const SERIES = [
        { key: "cpu", label: "CPU", color: "#38bdf8" },
        { key: "memory", label: "Memory", color: "#a855f7" },
        { key: "disk", label: "Disk", color: "#f59e0b" },
    ] as const;

    let { samples, capacity }: { samples: UsageSample[]; capacity: number } = $props();

    const gradientPrefix = `usage-${Math.random().toString(36).slice(2, 9)}`;
    const latest = $derived(samples.length ? samples[samples.length - 1] : null);

    function linePoints(key: (typeof SERIES)[number]["key"]): string[] {
        const step = WIDTH / (capacity - 1);
        const offset = capacity - samples.length;
        return samples.map((sample, index) => {
            const x = (offset + index) * step;
            const y = HEIGHT - (Math.min(sample[key], MAX_PERCENT) / MAX_PERCENT) * HEIGHT;
            return `${x.toFixed(1)},${y.toFixed(1)}`;
        });
    }

    const paths = $derived(
        SERIES.map((series) => {
            const points = linePoints(series.key);
            if (points.length < 2) return { ...series, line: "", area: "" };
            const line = `M ${points.join(" L ")}`;
            const firstX = points[0].split(",")[0];
            const lastX = points[points.length - 1].split(",")[0];
            return { ...series, line, area: `${line} L ${lastX},${HEIGHT} L ${firstX},${HEIGHT} Z` };
        }),
    );
</script>

<div class="flex flex-col gap-2">
    <div class="flex items-center justify-between gap-2 text-xs">
        {#each SERIES as series (series.key)}
            <span class="flex items-center gap-1 tabular-nums" style="color: {series.color}">
                <span class="size-1.5 rounded-full" style="background-color: {series.color}"></span>
                {series.label}
                {latest ? `${latest[series.key].toFixed(0)}%` : "-"}
            </span>
        {/each}
    </div>
    <svg viewBox="0 0 {WIDTH} {HEIGHT}" width="100%" height={HEIGHT} preserveAspectRatio="none" class="block rounded-md bg-muted/30">
        <defs>
            {#each SERIES as series (series.key)}
                <linearGradient id="{gradientPrefix}-{series.key}" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color={series.color} stop-opacity="0.3" />
                    <stop offset="100%" stop-color={series.color} stop-opacity="0" />
                </linearGradient>
            {/each}
        </defs>
        {#each paths as path (path.key)}
            {#if path.line}
                <path d={path.area} fill="url(#{gradientPrefix}-{path.key})" stroke="none" />
                <path d={path.line} fill="none" stroke={path.color} stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke" />
            {/if}
        {/each}
    </svg>
</div>
