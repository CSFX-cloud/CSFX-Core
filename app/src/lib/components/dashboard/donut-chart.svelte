<script lang="ts">
    import { PieChart } from "layerchart";
    import * as Chart from "$lib/components/ui/chart/index.js";
    import type { Segment } from "$lib/utils/dashboard";

    let { segments, total, label }: { segments: Segment[]; total: string; label: string } = $props();

    const config = $derived(
        Object.fromEntries(segments.map((s) => [s.label, { label: s.label, color: s.color }])) satisfies Chart.ChartConfig,
    );
    const data = $derived(segments.filter((s) => s.value > 0));
    const empty = $derived(data.length === 0);
</script>

<div class="relative shrink-0 size-36">
    <Chart.Container {config} class="size-full aspect-square">
        <PieChart
            data={empty ? [{ label: "empty", value: 1, color: "var(--muted)" }] : data}
            key="label"
            value="value"
            c="color"
            innerRadius={48}
            padding={8}
            props={{ pie: { motion: "tween" } }}
        >
            {#snippet tooltip()}
                {#if !empty}
                    <Chart.Tooltip hideLabel />
                {/if}
            {/snippet}
        </PieChart>
    </Chart.Container>
    <div class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
        <span class="text-xl font-semibold leading-tight">{total}</span>
        <span class="text-xs text-muted-foreground">{label}</span>
    </div>
</div>
