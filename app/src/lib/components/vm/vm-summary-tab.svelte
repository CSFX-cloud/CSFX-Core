<script lang="ts">
    import { untrack } from "svelte";
    import type { UsageHistoryPoint } from "$lib/api/nodes";
    import type { Workload } from "$lib/api/resource-groups";
    import UsageChart from "$lib/components/dashboard/usage-chart.svelte";
    import { chartCeiling, usageTone, USAGE_SERIES } from "$lib/utils/dashboard";
    import { formatBytes, formatCapacity, formatPercent } from "$lib/utils/format";

    const SAMPLE_CAPACITY = 60;
    const PERCENT = 100;

    let { workload, nodeIp }: { workload: Workload; nodeIp: string | null } = $props();

    let samples = $state<UsageHistoryPoint[]>([]);

    const cpu = $derived(workload.cpu_usage_percent);
    const memory = $derived(workload.memory_usage_bytes !== null && workload.memory_bytes > 0 ? (workload.memory_usage_bytes / workload.memory_bytes) * PERCENT : null);
    const ceiling = $derived(chartCeiling(samples, ["cpu", "memory"]));
    const latest = $derived(samples.length ? samples[samples.length - 1] : null);
    const glance = $derived([
        { label: "Desired state", value: workload.desired_state, mono: false },
        { label: "Restart policy", value: workload.restart_policy, mono: false },
        { label: "Node", value: nodeIp ?? workload.assigned_agent_id?.slice(0, 8) ?? "—", mono: true },
        { label: "Created", value: workload.created_at.slice(0, 16).replace("T", " "), mono: true },
    ]);

    $effect(() => {
        const cpuUsage = workload.cpu_usage_percent;
        const memoryBytes = workload.memory_usage_bytes;
        void workload.stats_updated_at;
        if (cpuUsage === null || memoryBytes === null) return;
        const point: UsageHistoryPoint = {
            bucket: new Date().toISOString(),
            cpu: cpuUsage,
            memory: workload.memory_bytes > 0 ? (memoryBytes / workload.memory_bytes) * PERCENT : 0,
            disk: 0,
        };
        samples = [...untrack(() => samples), point].slice(-SAMPLE_CAPACITY);
    });
</script>

{#snippet meter(percent: number | null, note: string)}
    <div class="cx-stat__foot">
        {#if percent !== null}
            <div class="cx-usage {usageTone(percent) ? `cx-usage--${usageTone(percent)}` : ''}" style="flex: 1">
                <div class="cx-usage__track" style="max-width: none"><div class="cx-usage__fill" style="width: {Math.min(percent, 100)}%"></div></div>
            </div>
        {/if}
        {note}
    </div>
{/snippet}

<div style="display: grid; gap: 12px">
    <section class="db-stats" aria-label="Usage">
        <div class="cx-stat">
            <p class="cx-stat__label">CPU</p>
            <p class="cx-stat__value stat">{cpu !== null ? formatPercent(cpu) : "—"}</p>
            {@render meter(cpu, `${workload.cpu_millicores / 1000} vCPU`)}
        </div>
        <div class="cx-stat">
            <p class="cx-stat__label">Memory</p>
            <p class="cx-stat__value stat">{memory !== null ? formatPercent(memory) : "—"}</p>
            {@render meter(memory, `${workload.memory_usage_bytes !== null ? `${formatBytes(workload.memory_usage_bytes)} / ` : ""}${formatCapacity(workload.memory_bytes)}`)}
        </div>
        <div class="cx-stat">
            <p class="cx-stat__label">Disk</p>
            <p class="cx-stat__value stat">{formatCapacity(workload.disk_bytes)}</p>
            <div class="cx-stat__foot">Allocated</div>
        </div>
        <div class="cx-stat">
            <p class="cx-stat__label">Network</p>
            <p class="db-stat-value" style="margin-top: 4px">
                {workload.network_rx_bytes !== null ? formatBytes(workload.network_rx_bytes) : "—"} <small>in</small> &middot;
                {workload.network_tx_bytes !== null ? formatBytes(workload.network_tx_bytes) : "—"} <small>out</small>
            </p>
            <div class="cx-stat__foot">{workload.stats_updated_at ? `Updated ${new Date(workload.stats_updated_at).toLocaleTimeString()}` : "No stats reported yet"}</div>
        </div>
    </section>

    <div class="vm-grid">
        <section class="cx-panel" aria-labelledby="vm-hist">
            <div class="cx-panel__head">
                <div><h2 class="cx-panel__title" id="vm-hist">Usage history</h2><p class="cx-panel__sub">Live, since you opened this page</p></div>
            </div>
            {#if samples.length < 2}
                <p class="cx-hint" style="padding: 32px 0; text-align: center">{workload.status === "running" ? "Collecting samples." : "No stats while the VM is stopped."}</p>
            {:else}
                <div class="db-legend">
                    {#each USAGE_SERIES as series (series.key)}
                        <span><i style="background: {series.color}"></i>{series.label} {formatPercent(latest?.[series.key])}</span>
                    {/each}
                </div>
                <UsageChart history={samples} range="1h" yMax={ceiling} label="CPU and memory of this VM" />
            {/if}
        </section>
        <section class="cx-panel" aria-labelledby="vm-sum">
            <div class="cx-panel__head"><h2 class="cx-panel__title" id="vm-sum">At a glance</h2></div>
            <dl class="nd-kv">
                {#each glance as row (row.label)}
                    <div><dt>{row.label}</dt><dd class:mono={row.mono}>{row.value}</dd></div>
                {/each}
                {#if workload.restart_count > 0}
                    <div><dt>Restarts</dt><dd class="mono">{workload.restart_count}</dd></div>
                {/if}
            </dl>
        </section>
    </div>
</div>
