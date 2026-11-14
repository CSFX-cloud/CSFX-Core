<script lang="ts">
    import { onMount } from "svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getSiteUsage, type SiteMode, type SiteUsage } from "$lib/api/sites";
    import { formatBytes } from "$lib/utils/format";

    const WINDOW_DAYS = 7;

    type Props = { rgId: string; siteId: string; mode: SiteMode };

    let { rgId, siteId, mode }: Props = $props();

    let usage = $state<SiteUsage | null>(null);
    let failed = $state(false);

    const measured = $derived(mode === "static");
    const requests = $derived(measured && usage ? usage.requests.toLocaleString() : "—");
    const bandwidth = $derived(measured && usage ? formatBytes(usage.bytes_out) : "—");
    const buildMinutes = $derived(usage ? usage.build_minutes.toFixed(1) : "—");

    onMount(async () => {
        if (!auth.token) return;
        try {
            usage = await getSiteUsage(rgId, siteId, WINDOW_DAYS);
        } catch {
            failed = true;
        }
    });
</script>

<section class="db-stats" aria-label="Usage in the last {WINDOW_DAYS} days">
    <div class="cx-stat">
        <p class="cx-stat__label">Requests</p>
        <p class="db-stat-value">{requests}</p>
        <div class="cx-stat__foot">{measured ? `last ${WINDOW_DAYS} days` : "served by the container"}</div>
    </div>
    <div class="cx-stat">
        <p class="cx-stat__label">Bandwidth</p>
        <p class="db-stat-value">{bandwidth}</p>
        <div class="cx-stat__foot">{measured ? `last ${WINDOW_DAYS} days` : "served by the container"}</div>
    </div>
    <div class="cx-stat">
        <p class="cx-stat__label">Build minutes</p>
        <p class="db-stat-value">{buildMinutes}</p>
        <div class="cx-stat__foot">{failed ? "unavailable" : `last ${WINDOW_DAYS} days`}</div>
    </div>
</section>
