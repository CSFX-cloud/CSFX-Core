<script lang="ts">
    import type { AlertEvent } from "$lib/api/events";
    import { getUsageHistory, type Node, type NodeMetricsLatest, type UsageHistoryPoint, type UsageRange } from "$lib/api/nodes";
    import { auth } from "$lib/auth/store.svelte";
    import UsageChart from "$lib/components/dashboard/usage-chart.svelte";
    import TrafficChart from "$lib/components/nodes/detail/traffic-chart.svelte";
    import type { HistoryPoint } from "$lib/components/nodes/detail/node-live.svelte";
    import { alertTitle, alertTone, chartCeiling, relativeTime, usageTone, USAGE_SERIES } from "$lib/utils/dashboard";
    import { formatBytes, formatCapacity, formatPercent, formatRate } from "$lib/utils/format";
    import { formatHeartbeat } from "$lib/utils/nodes";
    import { resolvedPercent } from "$lib/utils/node-detail";

    const RANGES: { value: UsageRange; label: string }[] = [
        { value: "1h", label: "1H" },
        { value: "24h", label: "24H" },
        { value: "7d", label: "7D" },
    ];
    const USAGE_INTERVAL_MS = 30000;
    const VISIBLE_ALERTS = 5;

    type Props = {
        node: Node;
        metrics: NodeMetricsLatest | null;
        history: HistoryPoint[];
        openAlerts: AlertEvent[];
        alertsLoading: boolean;
        onviewalerts: () => void;
    };

    let { node, metrics, history, openAlerts, alertsLoading, onviewalerts }: Props = $props();

    let range = $state<UsageRange>("1h");
    let usageHistory = $state<UsageHistoryPoint[]>([]);

    const cpu = $derived(metrics?.cpu_usage_percent ?? null);
    const memory = $derived(resolvedPercent(metrics?.memory_usage_percent, metrics?.memory_used_bytes, metrics?.memory_total_bytes));
    const disk = $derived(resolvedPercent(metrics?.disk_usage_percent, metrics?.disk_used_bytes, metrics?.disk_total_bytes));
    const ceiling = $derived(chartCeiling(usageHistory, ["cpu", "memory"]));
    const lastUsage = $derived(usageHistory.length ? usageHistory[usageHistory.length - 1] : null);
    const rates = $derived(history.length ? history[history.length - 1] : null);

    $effect(() => {
        const token = auth.token;
        const selected = range;
        if (!token) return;
        const load = async () => {
            const points = await getUsageHistory(token, selected, node.id).catch(() => []);
            if (selected === range) usageHistory = points;
        };
        load();
        const timer = setInterval(load, USAGE_INTERVAL_MS);
        return () => clearInterval(timer);
    });

    const details = $derived([
        { label: "ID", value: node.id.slice(0, 8), mono: true },
        { label: "OS", value: `${node.os_type} ${node.os_version}`, mono: false },
        { label: "Architecture", value: node.architecture, mono: true },
        { label: "Agent", value: `v${node.agent_version}`, mono: true },
        { label: "Registered", value: formatHeartbeat(node.registered_at), mono: true },
        { label: "Last heartbeat", value: formatHeartbeat(node.last_heartbeat), mono: true },
    ]);
</script>

{#snippet meter(percent: number | null, note: string)}
    <div class="cx-stat__foot">
        {#if percent !== null}
            <div class="cx-usage {usageTone(percent) ? `cx-usage--${usageTone(percent)}` : ''}" style="flex: 1">
                <div class="cx-usage__track" style="max-width: none"><div class="cx-usage__fill" style="width: {percent}%"></div></div>
            </div>
        {/if}
        {note}
    </div>
{/snippet}

<div style="display: grid; gap: 16px">
    <section class="db-stats" aria-label="Current usage">
        <div class="cx-stat">
            <p class="cx-stat__label">CPU</p>
            <p class="cx-stat__value stat">{formatPercent(cpu)}</p>
            {@render meter(cpu, `${metrics?.cpu_cores ?? "-"} cores`)}
        </div>
        <div class="cx-stat">
            <p class="cx-stat__label">Memory</p>
            <p class="cx-stat__value stat">{formatPercent(memory)}</p>
            {@render meter(memory, `${formatCapacity(metrics?.memory_used_bytes)} / ${formatCapacity(metrics?.memory_total_bytes)}`)}
        </div>
        <div class="cx-stat">
            <p class="cx-stat__label">Disk</p>
            <p class="cx-stat__value stat">{formatPercent(disk)}</p>
            {@render meter(disk, `of ${formatCapacity(metrics?.disk_total_bytes)}`)}
        </div>
        <div class="cx-stat">
            <p class="cx-stat__label">Network</p>
            <p class="db-stat-value" style="margin-top: 4px">{formatRate(rates?.rxBps ?? 0)} <small>in</small> &middot; {formatRate(rates?.txBps ?? 0)} <small>out</small></p>
            <div class="cx-stat__foot">RX {formatBytes(metrics?.network_rx_bytes)} &middot; TX {formatBytes(metrics?.network_tx_bytes)} total</div>
        </div>
    </section>

    <div class="nd-grid">
        <div class="nd-col">
            <section class="cx-panel" aria-labelledby="nd-usage">
                <div class="cx-panel__head">
                    <div>
                        <h2 class="cx-panel__title" id="nd-usage">Usage history</h2>
                        <p class="cx-panel__sub">CPU and memory on this node</p>
                    </div>
                    <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Time range">
                        {#each RANGES as entry (entry.value)}
                            <button class="cx-tab" role="tab" type="button" aria-selected={entry.value === range} onclick={() => (range = entry.value)}>
                                {entry.label}
                            </button>
                        {/each}
                    </div>
                </div>
                <div class="db-legend">
                    {#each USAGE_SERIES as series (series.key)}
                        <span><i style="background: {series.color}"></i>{series.label} {formatPercent(lastUsage?.[series.key])}</span>
                    {/each}
                </div>
                <UsageChart history={usageHistory} {range} yMax={ceiling} label="CPU and memory usage of this node over the selected range" />
            </section>

            <section class="cx-panel" aria-labelledby="nd-net">
                <div class="cx-panel__head">
                    <div>
                        <h2 class="cx-panel__title" id="nd-net">Network traffic</h2>
                        <p class="cx-panel__sub">Receive and transmit, live</p>
                    </div>
                </div>
                <TrafficChart points={history} />
            </section>
        </div>

        <div class="nd-col">
            <section class="cx-panel" aria-labelledby="nd-alerts-summary">
                <div class="cx-panel__head">
                    <div>
                        <h2 class="cx-panel__title" id="nd-alerts-summary">Alerts</h2>
                        <p class="cx-panel__sub">{alertsLoading ? "Loading events" : `${openAlerts.length} open`}</p>
                    </div>
                    <button class="cx-link db-link-button" type="button" onclick={onviewalerts}>View all</button>
                </div>
                <div class="db-alerts__list">
                    {#if alertsLoading}
                        {#each { length: 2 } as _, index (index)}<span class="db-skeleton db-skeleton--alert" aria-hidden="true"></span>{/each}
                    {:else if openAlerts.length === 0}
                        <p class="cx-hint">No open alerts</p>
                    {:else}
                        {#each openAlerts.slice(0, VISIBLE_ALERTS) as event (event.id)}
                            <div class="cx-alert cx-alert--{alertTone(event)}">
                                <span class="cx-alert__dot"></span>
                                <span class="cx-alert__title">{alertTitle(event)}</span>
                                <span class="cx-alert__time">{relativeTime(event.created_at, Date.now())}</span>
                                <span class="cx-alert__body">{event.message ?? event.event_type}</span>
                            </div>
                        {/each}
                    {/if}
                </div>
            </section>

            <section class="cx-panel" aria-labelledby="nd-details">
                <div class="cx-panel__head"><h2 class="cx-panel__title" id="nd-details">Details</h2></div>
                <dl class="nd-kv">
                    {#each details as detail (detail.label)}
                        <div><dt>{detail.label}</dt><dd class:mono={detail.mono}>{detail.value}</dd></div>
                    {/each}
                </dl>
            </section>
        </div>
    </div>
</div>
