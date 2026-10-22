<script lang="ts">
    import type { Node, NodeMetricsLatest } from "$lib/api/nodes";
    import TrafficChart from "$lib/components/nodes/detail/traffic-chart.svelte";
    import type { HistoryPoint } from "$lib/components/nodes/detail/node-live.svelte";
    import { formatBytes, formatRate } from "$lib/utils/format";

    type Props = { node: Node; metrics: NodeMetricsLatest; history: HistoryPoint[] };

    let { node, metrics, history }: Props = $props();

    const latest = $derived(history.length ? history[history.length - 1] : null);
    const entries = $derived([
        { label: "IP address", value: node.ip_address ?? "-" },
        { label: "Receive", value: formatRate(latest?.rxBps ?? 0) },
        { label: "Transmit", value: formatRate(latest?.txBps ?? 0) },
        { label: "Total received", value: formatBytes(metrics.network_rx_bytes) },
        { label: "Total sent", value: formatBytes(metrics.network_tx_bytes) },
    ]);
</script>

<div class="nd-grid">
    <section class="cx-panel" aria-labelledby="nd-nw-traffic">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="nd-nw-traffic">Traffic</h2>
                <p class="cx-panel__sub">Primary interface, live</p>
            </div>
        </div>
        <TrafficChart points={history} height={180} />
    </section>
    <section class="cx-panel" aria-labelledby="nd-nw-interface">
        <div class="cx-panel__head"><h2 class="cx-panel__title" id="nd-nw-interface">Interface</h2></div>
        <dl class="nd-kv">
            {#each entries as entry (entry.label)}
                <div><dt>{entry.label}</dt><dd class="mono">{entry.value}</dd></div>
            {/each}
        </dl>
    </section>
</div>
