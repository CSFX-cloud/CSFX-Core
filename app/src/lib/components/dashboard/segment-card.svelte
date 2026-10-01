<script lang="ts">
    import type { Segment } from "$lib/utils/dashboard";
    import DonutChart from "./donut-chart.svelte";

    let {
        title,
        totalLabel,
        segments,
        loading,
    }: { title: string; totalLabel: string; segments: Segment[]; loading: boolean } = $props();

    const total = $derived(segments.reduce((acc, s) => acc + s.value, 0));
</script>

<section class="flex items-center gap-6 min-w-0">
    <DonutChart {segments} total={loading ? "-" : String(total)} label={totalLabel} />
    <div class="flex-1 min-w-0">
        <p class="text-sm font-medium mb-3">{title}</p>
        <div class="flex flex-col gap-2">
            {#each segments as segment (segment.label)}
                <div class="flex items-center gap-2.5 text-sm">
                    <span class="size-2 rounded-full shrink-0" style="background-color: {segment.color}"></span>
                    <span class="flex-1 truncate text-muted-foreground">{segment.label}</span>
                    <span class="font-medium tabular-nums">{loading ? "-" : segment.value}</span>
                </div>
            {/each}
        </div>
    </div>
</section>
