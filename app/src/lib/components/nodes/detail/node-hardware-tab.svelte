<script lang="ts">
    import type { Node, NodeMetricsLatest } from "$lib/api/nodes";
    import { usageTone } from "$lib/utils/dashboard";
    import { formatCapacity, formatPercent, formatUptime } from "$lib/utils/format";
    import { formatHeartbeat } from "$lib/utils/nodes";
    import { resolvedPercent } from "$lib/utils/node-detail";

    type Entry = { label: string; value?: string | number | null; mono?: boolean; percent?: number | null };
    type Section = { id: string; title: string; entries: Entry[] };

    let { node, metrics }: { node: Node; metrics: NodeMetricsLatest } = $props();

    const sections = $derived<Section[]>([
        {
            id: "host",
            title: "Host",
            entries: [
                { label: "Hostname", value: node.hostname },
                { label: "OS", value: `${node.os_type} ${node.os_version}` },
                { label: "Kernel", value: metrics.kernel_version, mono: true },
                { label: "Architecture", value: node.architecture, mono: true },
                { label: "Uptime", value: formatUptime(metrics.uptime_seconds) },
                { label: "Registered", value: formatHeartbeat(node.registered_at), mono: true },
                { label: "Last heartbeat", value: formatHeartbeat(node.last_heartbeat), mono: true },
            ],
        },
        {
            id: "cpu",
            title: "CPU",
            entries: [
                { label: "Model", value: metrics.cpu_model },
                { label: "Cores", value: metrics.cpu_cores, mono: true },
                { label: "Threads", value: metrics.cpu_threads, mono: true },
                { label: "Usage", percent: metrics.cpu_usage_percent },
            ],
        },
        {
            id: "memory",
            title: "Memory",
            entries: [
                { label: "Total", value: formatCapacity(metrics.memory_total_bytes), mono: true },
                { label: "Used", value: formatCapacity(metrics.memory_used_bytes), mono: true },
                { label: "Usage", percent: resolvedPercent(metrics.memory_usage_percent, metrics.memory_used_bytes, metrics.memory_total_bytes) },
            ],
        },
        {
            id: "storage",
            title: "Storage",
            entries: [
                { label: "Total", value: formatCapacity(metrics.disk_total_bytes), mono: true },
                { label: "Used", value: formatCapacity(metrics.disk_used_bytes), mono: true },
                { label: "Usage", percent: resolvedPercent(metrics.disk_usage_percent, metrics.disk_used_bytes, metrics.disk_total_bytes) },
            ],
        },
    ]);
</script>

<div class="nd-two">
    {#each sections as section (section.id)}
        <section class="cx-panel" aria-labelledby="nd-hw-{section.id}">
            <div class="cx-panel__head"><h2 class="cx-panel__title" id="nd-hw-{section.id}">{section.title}</h2></div>
            <dl class="nd-kv">
                {#each section.entries as entry (entry.label)}
                    <div>
                        <dt>{entry.label}</dt>
                        {#if "percent" in entry}
                            <dd>
                                {#if entry.percent == null}
                                    <span class="na">Not reported</span>
                                {:else}
                                    <div class="cx-usage {usageTone(entry.percent) ? `cx-usage--${usageTone(entry.percent)}` : ''}" style="justify-content: flex-end">
                                        <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {entry.percent}%"></div></div>
                                        <span class="cx-usage__value">{formatPercent(entry.percent)}</span>
                                    </div>
                                {/if}
                            </dd>
                        {:else if entry.value == null || entry.value === ""}
                            <dd class="na">Not reported</dd>
                        {:else}
                            <dd class:mono={entry.mono}>{entry.value}</dd>
                        {/if}
                    </div>
                {/each}
            </dl>
        </section>
    {/each}
</div>
