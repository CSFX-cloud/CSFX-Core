<script lang="ts">
    import type { ClusterStats } from "$lib/api/nodes";
    import { formatCapacity } from "$lib/utils/format";
    import { ratio } from "$lib/utils/nodes";
    import { usageTone } from "$lib/utils/dashboard";

    let { stats }: { stats: ClusterStats | null } = $props();

    const offline = $derived(stats ? stats.node_count - stats.online_count : 0);
    const meters = $derived([
        { label: "vCPU", value: stats ? String(stats.total_cpu_cores) : "-", total: null, percent: stats?.avg_cpu_usage_percent ?? null },
        {
            label: "Memory",
            value: formatCapacity(stats?.used_memory_bytes),
            total: formatCapacity(stats?.total_memory_bytes),
            percent: ratio(stats?.used_memory_bytes, stats?.total_memory_bytes),
        },
        {
            label: "Disk",
            value: formatCapacity(stats?.used_disk_bytes),
            total: formatCapacity(stats?.total_disk_bytes),
            percent: ratio(stats?.used_disk_bytes, stats?.total_disk_bytes),
        },
    ]);
</script>

<section class="db-stats" aria-label="Cluster capacity">
    <div class="cx-stat">
        <p class="cx-stat__label">Nodes online</p>
        <p class="cx-stat__value stat">
            {stats ? stats.online_count : "-"}{#if stats} <small>/ {stats.node_count}</small>{/if}
        </p>
        <div class="cx-stat__foot">
            {#if offline > 0}
                <span class="cx-badge cx-badge--danger">{offline} offline</span>
            {/if}
        </div>
    </div>
    {#each meters as meter (meter.label)}
        <div class="cx-stat">
            <p class="cx-stat__label">{meter.label}</p>
            <p class="db-stat-value">{meter.value}{#if meter.total} <small>/ {meter.total}</small>{/if}</p>
            <div class="cx-stat__foot">
                {#if meter.percent !== null}
                    <div class="cx-usage {usageTone(meter.percent) ? `cx-usage--${usageTone(meter.percent)}` : ''}" style="flex: 1">
                        <div class="cx-usage__track" style="max-width: none"><div class="cx-usage__fill" style="width: {meter.percent}%"></div></div>
                        <span class="cx-usage__value">{meter.percent.toFixed(1)}%</span>
                    </div>
                {/if}
            </div>
        </div>
    {/each}
</section>
