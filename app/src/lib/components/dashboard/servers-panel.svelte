<script lang="ts">
    import { RefreshCw, ServerOff } from "@lucide/svelte";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import type { Node, NodeMetrics } from "$lib/api/nodes";
    import { nodeTone, usageTone } from "$lib/utils/dashboard";

    const PERCENT = 100;

    let { nodes, metrics, loading }: { nodes: Node[]; metrics: NodeMetrics[]; loading: boolean } = $props();

    const metricsById = $derived(new Map(metrics.map((entry) => [entry.agent_id, entry])));

    function memoryPercent(entry: NodeMetrics | undefined): number | null {
        if (!entry?.memory_total_bytes || entry.memory_used_bytes === null) return null;
        return (entry.memory_used_bytes / entry.memory_total_bytes) * PERCENT;
    }

    function label(status: string): string {
        return status.charAt(0).toUpperCase() + status.slice(1).toLowerCase();
    }
</script>

{#snippet usage(percent: number | null)}
    {#if percent === null}
        <span class="cx-num">&mdash;</span>
    {:else}
        <div class="cx-usage {usageTone(percent) ? `cx-usage--${usageTone(percent)}` : ''}">
            <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {percent}%"></div></div>
            <span class="cx-usage__value">{percent.toFixed(0)}%</span>
        </div>
    {/if}
{/snippet}

<section class="cx-panel" aria-labelledby="db-servers-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="db-servers-title">Servers</h2>
            <p class="cx-panel__sub">Hypervisor nodes in this cluster</p>
        </div>
        <span class="cx-panel__meta"><RefreshCw class="cx-i" aria-hidden="true" />Live</span>
    </div>
    {#if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: 3 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if nodes.length === 0}
        <EmptyState icon={ServerOff} label="No servers registered" />
    {:else}
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Server</th><th>Status</th><th>Address</th><th>CPU</th><th>Memory</th><th>Role</th></tr></thead>
                <tbody>
                    {#each nodes as node (node.id)}
                        {@const entry = metricsById.get(node.id)}
                        <tr>
                            <td><a href="/nodes/{node.id}" style="color: inherit; text-decoration: none">{node.name}</a></td>
                            <td><span class="cx-badge cx-badge--{nodeTone(node.status)}">{label(node.status)}</span></td>
                            <td class="cx-num">{node.ip_address ?? "—"}</td>
                            <td>{@render usage(entry?.cpu_usage_percent ?? null)}</td>
                            <td>{@render usage(memoryPercent(entry))}</td>
                            <td class="cx-soft">{label(node.role)}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</section>
