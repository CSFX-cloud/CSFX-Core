<script lang="ts">
    import { Box, ChevronRight } from "@lucide/svelte";
    import type { Workload } from "$lib/api/resource-groups";
    import { usageTone } from "$lib/utils/dashboard";
    import { formatBytes } from "$lib/utils/format";
    import { workloadTone } from "$lib/utils/node-detail";

    const SKELETON_ROWS = 3;

    type Props = { hostname: string; workloads: Workload[]; loading: boolean; error: string | null };

    let { hostname, workloads, loading, error }: Props = $props();

    function label(status: string): string {
        return status.charAt(0).toUpperCase() + status.slice(1).toLowerCase();
    }
</script>

{#if error}
    <p class="cx-hint cx-hint--error">{error}</p>
{:else if loading}
    <section class="cx-panel" aria-busy="true" aria-label="Workloads">
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    </section>
{:else if workloads.length === 0}
    <section class="cx-stage nd-empty" aria-label="Workloads">
        <div class="cx-stage__canvas"></div>
        <div class="nd-empty__in">
            <span class="nd-tile" aria-hidden="true"><Box class="cx-i" /></span>
            <h2 class="nd-empty__title">No workloads on this node.</h2>
            <p class="nd-empty__text">VMs and containers the orchestrator places on {hostname} appear here.</p>
            <a class="cx-btn cx-btn--secondary cx-btn--sm" href="/resource-groups" style="margin-top: 8px">
                Create VM
                <ChevronRight class="cx-i cx-chev" aria-hidden="true" />
            </a>
        </div>
    </section>
{:else}
    <section class="cx-panel" aria-labelledby="nd-workloads">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="nd-workloads">Workloads</h2>
                <p class="cx-panel__sub">{workloads.length} scheduled on {hostname}</p>
            </div>
        </div>
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Name</th><th>Image</th><th>Status</th><th>CPU</th><th>Memory</th></tr></thead>
                <tbody>
                    {#each workloads as workload (workload.id)}
                        <tr>
                            <td>{workload.name}</td>
                            <td class="cx-num">{workload.image}</td>
                            <td><span class="cx-badge cx-badge--{workloadTone(workload.status)}">{label(workload.status)}</span></td>
                            <td>
                                {#if workload.cpu_usage_percent == null}
                                    <span class="cx-num">&mdash;</span>
                                {:else}
                                    <div class="cx-usage {usageTone(workload.cpu_usage_percent) ? `cx-usage--${usageTone(workload.cpu_usage_percent)}` : ''}">
                                        <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {workload.cpu_usage_percent}%"></div></div>
                                        <span class="cx-usage__value">{workload.cpu_usage_percent.toFixed(0)}%</span>
                                    </div>
                                {/if}
                            </td>
                            <td class="cx-num">{workload.memory_usage_bytes == null ? "—" : formatBytes(workload.memory_usage_bytes)}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    </section>
{/if}
