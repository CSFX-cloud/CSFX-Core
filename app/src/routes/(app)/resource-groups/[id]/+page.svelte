<script lang="ts">
    import { goto } from "$app/navigation";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import { Button } from "$lib/components/ui/button/index.js";
    import type { Workload } from "$lib/api/resource-groups";
    import BucketDetailDialog from "$lib/components/resource-group/bucket-detail-dialog.svelte";
    import ResourceTable from "$lib/components/resource-group/resource-table.svelte";
    import RgTopbar from "$lib/components/resource-group/rg-topbar.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let bucketDetailDialog = $state<BucketDetailDialog | null>(null);

    function openWorkload(workload: Workload) {
        const kind = workload.runtime_class === "vm" ? "vm" : "container";
        goto(`${base}/${kind}/${workload.id}`);
    }
</script>

<BucketDetailDialog bind:this={bucketDetailDialog} rgId={rg.rgId} />

<RgTopbar backHref="/resource-groups" backLabel="Back to resource groups">
    {#snippet actions()}
        <Button onclick={() => goto(`${base}/add`)}>
            <PlusIcon class="size-4" />
            Add Resource
        </Button>
    {/snippet}
</RgTopbar>

<div class="flex min-w-0 flex-1 flex-col gap-6 p-6">
    {#if rg.loading}
        <p class="text-sm text-muted-foreground">Loading...</p>
    {:else if rg.error && !rg.group}
        <p class="text-sm text-destructive">{rg.error}</p>
    {:else if rg.group}
        {#if rg.error}
            <p class="text-xs text-destructive">{rg.error}</p>
        {/if}

        <ResourceTable
            workloads={rg.workloads}
            volumes={rg.volumes}
            buckets={rg.buckets}
            onOpenWorkload={openWorkload}
            onOpenStack={(stackId) => goto(`${base}/stack/${stackId}`)}
            onOpenBucket={(bucket) => bucketDetailDialog?.open(bucket)}
            onChanged={rg.load}
            onError={(message) => (rg.error = message)}
        />
    {/if}
</div>
