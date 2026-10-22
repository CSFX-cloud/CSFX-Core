<script lang="ts">
    import { untrack } from "svelte";
    import { ChevronRight, Database, Plus, RefreshCw } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { listBuckets, listResourceGroups, type Bucket } from "$lib/api/resource-groups";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { badgeClass, statusLabel } from "$lib/utils/tone";
    import { fmtBytes } from "$lib/utils/format";

    const SKELETON_ROWS = 3;

    let buckets = $state<Bucket[]>([]);
    let groupNames = $state(new Map<string, string>());
    let loading = $state(true);
    let refreshing = $state(false);
    let error = $state<string | null>(null);

    async function refresh() {
        if (!auth.token || refreshing) return;
        refreshing = true;
        try {
            const [loaded, groups] = await Promise.all([listBuckets(auth.token), listResourceGroups(auth.token).catch(() => [])]);
            buckets = loaded;
            groupNames = new Map(groups.map((group) => [group.id, group.name]));
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load buckets";
        } finally {
            refreshing = false;
            loading = false;
        }
    }

    $effect(() => {
        if (auth.token) untrack(refresh);
    });
</script>

<div class="db-content">
    <div class="db-head">
        <div>
            <h1 class="db-title">S3 buckets</h1>
            <p class="db-sub">Object storage across all resource groups.</p>
        </div>
        <div class="db-actions">
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={refreshing} aria-busy={refreshing} onclick={refresh}>
                <RefreshCw class="cx-i {refreshing ? 'db-spin' : ''}" aria-hidden="true" />
                Refresh
            </button>
        </div>
    </div>

    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if buckets.length === 0}
        <section class="cx-stage empty bk-empty" aria-label="No buckets">
            <div class="cx-stage__canvas"></div>
            <div class="empty__in">
                <span class="glow" aria-hidden="true"><Database class="cx-i" /></span>
                <h2>No buckets yet.</h2>
                <p>Buckets live inside a resource group and keep their data on this cluster. Any S3-compatible client can reach them.</p>
                <a class="cx-btn cx-btn--primary cx-btn--sm" href="/resource-groups" style="margin-top: 8px">
                    <Plus class="cx-i" aria-hidden="true" />
                    Create bucket
                </a>
            </div>
        </section>
    {:else}
        <section class="cx-panel" aria-label="All buckets">
            <div class="db-scroll">
                <table class="cx-table">
                    <thead>
                        <tr><th>Name</th><th>Global alias</th><th>Exposure</th><th>Quota</th><th>Status</th><th>Resource group</th><th><span class="db-sr-only">Open</span></th></tr>
                    </thead>
                    <tbody>
                        {#each buckets as bucket (bucket.id)}
                            <tr class="nl-row">
                                <td>
                                    <div class="rs-name">
                                        <ServiceIcon name="object-storage" />
                                        <a class="rs-link" href="/buckets/{bucket.id}">{bucket.name}</a>
                                    </div>
                                </td>
                                <td class="cx-num">{bucket.global_alias}</td>
                                <td class="cx-soft">{bucket.exposure}</td>
                                <td class="cx-soft">{bucket.quota_max_size ? fmtBytes(bucket.quota_max_size) : "unlimited"}</td>
                                <td><span class={badgeClass(bucket.status)}>{statusLabel(bucket.status)}</span></td>
                                <td class="cx-soft">
                                    {#if bucket.resource_group_id}
                                        <a class="cx-link rs-more" href="/resource-groups/{bucket.resource_group_id}">{groupNames.get(bucket.resource_group_id) ?? bucket.resource_group_id.slice(0, 8)}</a>
                                    {:else}
                                        &mdash;
                                    {/if}
                                </td>
                                <td><span class="nl-go"><ChevronRight class="cx-i" aria-hidden="true" /></span></td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </section>
    {/if}
</div>
