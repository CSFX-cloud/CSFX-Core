<script lang="ts">
    import { BellOff, Check } from "@lucide/svelte";
    import type { AlertEvent } from "$lib/api/events";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { alertTitle } from "$lib/utils/dashboard";

    const SKELETON_ROWS = 3;

    type Props = { alerts: AlertEvent[]; loading: boolean; error: string | null; onresolve: (id: string) => void };

    let { alerts, loading, error, onresolve }: Props = $props();

    function formatTime(value: string | null): string {
        return value ? new Date(value).toLocaleString() : "—";
    }
</script>

<section class="cx-panel" aria-labelledby="nd-al">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="nd-al">Alerts</h2>
            <p class="cx-panel__sub">Events reported by this node</p>
        </div>
    </div>
    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if alerts.length === 0}
        <EmptyState icon={BellOff} label="No alerts for this node" />
    {:else}
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Alert</th><th>Status</th><th>Raised</th><th>Resolved</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
                <tbody>
                    {#each alerts as alert (alert.id)}
                        <tr>
                            <td>
                                <div class="nl-host">
                                    <span style="font-weight: 500">{alertTitle(alert)}</span>
                                    <span>{alert.message ?? alert.event_type}</span>
                                </div>
                            </td>
                            <td><span class="cx-badge {alert.status === 'open' ? 'cx-badge--warning' : ''}">{alert.status}</span></td>
                            <td class="cx-num">{formatTime(alert.created_at)}</td>
                            <td class="cx-num">{formatTime(alert.resolved_at)}</td>
                            <td>
                                {#if alert.status === "open"}
                                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => onresolve(alert.id)}>
                                        <Check class="cx-i" aria-hidden="true" />
                                        Resolve
                                    </button>
                                {/if}
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</section>
