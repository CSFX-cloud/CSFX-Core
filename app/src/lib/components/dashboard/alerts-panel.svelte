<script lang="ts">
    import { BellOff } from "@lucide/svelte";
    import type { AlertEvent } from "$lib/api/events";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { alertHref, alertTitle, alertTone, parseTimestamp, relativeTime } from "$lib/utils/dashboard";

    const VISIBLE_ALERTS = 3;

    let { events, now, loading }: { events: AlertEvent[]; now: number; loading: boolean } = $props();

    const recent = $derived(
        [...events].sort((a, b) => parseTimestamp(b.created_at) - parseTimestamp(a.created_at)).slice(0, VISIBLE_ALERTS),
    );
</script>

<section class="cx-panel db-alerts" aria-labelledby="db-alerts-title" aria-busy={loading}>
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="db-alerts-title">Recent alerts</h2>
            <p class="cx-panel__sub">{loading ? "Loading events" : recent.length ? "Latest cluster events" : "No alerts yet"}</p>
        </div>
        <a class="cx-link" href="/monitoring" style="font-size: 13px">View all</a>
    </div>
    {#if loading}
        <div class="db-alerts__list">
            {#each { length: VISIBLE_ALERTS } as _, index (index)}
                <span class="db-skeleton db-skeleton--alert" aria-hidden="true"></span>
            {/each}
        </div>
    {:else if recent.length === 0}
        <EmptyState icon={BellOff} label="No alerts" />
    {:else}
        <div class="db-alerts__list">
            {#each recent as event (event.id)}
                <a class="cx-alert cx-alert--{alertTone(event)}" href={alertHref(event)}>
                    <span class="cx-alert__dot"></span>
                    <span class="cx-alert__title">
                        {alertTitle(event)}
                        {#if event.status === "open"}<span class="cx-alert__new">NEW</span>{/if}
                    </span>
                    <span class="cx-alert__time">{relativeTime(event.created_at, now)}</span>
                    <span class="cx-alert__body">{event.message ?? event.event_type}</span>
                </a>
            {/each}
        </div>
    {/if}
</section>
