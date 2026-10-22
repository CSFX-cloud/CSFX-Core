<script lang="ts">
    import { Box, ChevronRight, Database, HardDrive, Monitor } from "@lucide/svelte";
    import type { ServiceCounts } from "$lib/utils/dashboard";

    let { counts, loading }: { counts: ServiceCounts; loading: boolean } = $props();

    const hasWorkloads = $derived(counts.vms + counts.containers > 0);

    const rows = $derived([
        { label: "Virtual machines", href: "/resource-groups", icon: Monitor, value: counts.vms },
        { label: "Containers", href: "/resource-groups", icon: Box, value: counts.containers },
        { label: "Volumes", href: "/resource-groups", icon: HardDrive, value: counts.volumes },
        { label: "Buckets", href: "/buckets", icon: Database, value: counts.buckets },
    ]);
</script>

<section class="cx-stage db-empty db-services" aria-labelledby="db-services-title" aria-busy={loading}>
    <div class="cx-stage__canvas"></div>
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="db-services-title">Services</h2>
            <p class="cx-panel__sub">{loading ? "Loading services" : counts.total === 0 ? "Nothing running yet" : `${counts.total} in total`}</p>
        </div>
    </div>
    <div class="db-list">
        {#each rows as row (row.label)}
            <a class="db-row" href={row.href} style="text-decoration: none">
                <row.icon class="cx-i" aria-hidden="true" />
                <span class="db-row__name">{row.label}</span>
                {#if loading}<span class="db-skeleton db-skeleton--value" aria-hidden="true"></span>{:else}<span class="db-row__val">{row.value}</span>{/if}
            </a>
        {/each}
    </div>
    <a class="cx-btn cx-btn--secondary cx-btn--sm cx-btn--block" href="/resource-groups" style="margin-top: 16px">
        {hasWorkloads ? "Go to resource groups" : "Create your first VM"}
        <ChevronRight class="cx-i cx-chev" aria-hidden="true" />
    </a>
</section>
