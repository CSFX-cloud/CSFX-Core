<script lang="ts">
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { listBuckets, type Bucket } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import Icon from "@iconify/svelte";

    let buckets = $state<Bucket[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let refreshing = $state(false);

    function fmtBytes(bytes: number): string {
        if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
        if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(0)} MB`;
        return `${bytes} B`;
    }

    async function load() {
        if (!auth.token) return;
        try {
            buckets = await listBuckets(auth.token);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load buckets";
        } finally {
            loading = false;
        }
    }

    async function refresh() {
        if (!auth.token || refreshing) return;
        refreshing = true;
        try {
            buckets = await listBuckets(auth.token);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to refresh";
        } finally {
            refreshing = false;
        }
    }

    let loadStarted = false;
    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            load();
        }
    });
</script>

<div class="flex flex-col gap-6 p-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-xl font-semibold tracking-tight">S3 Buckets</h1>
            <p class="text-sm text-muted-foreground mt-0.5">Object storage buckets across all resource groups</p>
        </div>
        <Button
            variant="outline"
            size="sm"
            onclick={refresh}
            disabled={refreshing}
            class={refreshing ? "opacity-60" : ""}
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class={refreshing ? "animate-spin" : ""}
            >
                <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/>
                <path d="M21 3v5h-5"/>
                <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/>
                <path d="M8 16H3v5"/>
            </svg>
            Refresh
        </Button>
    </div>

    <div class="border rounded-lg overflow-hidden">
        <table class="w-full text-sm">
            <thead class="bg-muted/50">
                <tr>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Name</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Global Alias</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Exposure</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Quota</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Status</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Resource Group</th>
                </tr>
            </thead>
            <tbody>
                {#if loading}
                    <tr>
                        <td colspan="6" class="px-4 py-8 text-center text-muted-foreground">Loading...</td>
                    </tr>
                {:else if error}
                    <tr>
                        <td colspan="6" class="px-4 py-8 text-center text-destructive">{error}</td>
                    </tr>
                {:else if buckets.length === 0}
                    <tr>
                        <td colspan="6" class="px-4 py-8 text-center text-muted-foreground">No buckets found</td>
                    </tr>
                {:else}
                    {#each buckets as bucket (bucket.id)}
                        <tr
                            class="border-t hover:bg-muted/30 transition-colors cursor-pointer"
                            onclick={() => goto(`/buckets/${bucket.id}`)}
                        >
                            <td class="px-4 py-3">
                                <div class="flex items-center gap-2.5">
                                    <Icon icon="fluent-emoji-high-contrast:bucket" width={20} height={20} class="shrink-0" />
                                    <span class="font-medium">{bucket.name}</span>
                                </div>
                            </td>
                            <td class="px-4 py-3 text-muted-foreground font-mono text-xs">{bucket.global_alias}</td>
                            <td class="px-4 py-3 text-muted-foreground">{bucket.exposure}</td>
                            <td class="px-4 py-3 text-muted-foreground">
                                {bucket.quota_max_size ? fmtBytes(bucket.quota_max_size) : "unlimited"}
                            </td>
                            <td class="px-4 py-3">
                                <StatusBadge status={bucket.status} />
                            </td>
                            <td class="px-4 py-3">
                                {#if bucket.resource_group_id}
                                    <a
                                        href="/resource-groups/{bucket.resource_group_id}"
                                        class="text-xs text-muted-foreground hover:text-foreground font-mono underline underline-offset-2"
                                        onclick={(e) => e.stopPropagation()}
                                    >
                                        {bucket.resource_group_id.slice(0, 8)}
                                    </a>
                                {:else}
                                    <span class="text-xs text-muted-foreground">-</span>
                                {/if}
                            </td>
                        </tr>
                    {/each}
                {/if}
            </tbody>
        </table>
    </div>
</div>
