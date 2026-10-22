<script lang="ts">
    import { untrack } from "svelte";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { listNodes } from "$lib/api/nodes";
    import { deleteResourceGroup } from "$lib/api/resource-groups";
    import AppearanceDialog from "$lib/components/resource-group/appearance-dialog.svelte";
    import BucketDetailDialog from "$lib/components/resource-group/bucket-detail-dialog.svelte";
    import GroupHero from "$lib/components/resource-group/group-hero.svelte";
    import GroupStats from "$lib/components/resource-group/group-stats.svelte";
    import ResourceTable from "$lib/components/resource-group/resource-table.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let appearanceDialog = $state<AppearanceDialog | null>(null);
    let bucketDetailDialog = $state<BucketDetailDialog | null>(null);
    let nodeNames = $state(new Map<string, string>());

    async function loadNodeNames() {
        if (!auth.token) return;
        const nodes = await listNodes(auth.token).catch(() => []);
        nodeNames = new Map(nodes.map((node) => [node.id, node.hostname]));
    }

    async function deleteGroup() {
        if (!auth.token || !rg.group || !confirm(`Delete resource group ${rg.group.name}? All of its resources are removed.`)) return;
        try {
            await deleteResourceGroup(auth.token, rg.group.id);
            await goto("/resource-groups");
        } catch (e) {
            rg.error = e instanceof Error ? e.message : "Failed to delete resource group";
        }
    }

    $effect(() => {
        if (auth.token) untrack(loadNodeNames);
    });

    $effect(() => {
        breadcrumb.set(rg.group?.name ?? null);
        return () => breadcrumb.set(null);
    });
</script>

<AppearanceDialog bind:this={appearanceDialog} group={rg.group} onUpdated={(updated) => (rg.group = updated)} />
<BucketDetailDialog bind:this={bucketDetailDialog} rgId={rg.rgId} />

<div class="db-content">
    {#if rg.loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !rg.group}
        <p class="cx-hint cx-hint--error">{rg.error ?? "Resource group not found"}</p>
    {:else}
        <GroupHero
            group={rg.group}
            {base}
            onupdated={(updated) => (rg.group = updated)}
            onerror={(message) => (rg.error = message)}
            oneditappearance={() => appearanceDialog?.open(rg.group)}
            ondelete={deleteGroup}
        />

        {#if rg.error}<p class="cx-hint cx-hint--error">{rg.error}</p>{/if}

        <GroupStats workloads={rg.workloads} volumes={rg.volumes} buckets={rg.buckets} cidr={rg.group.internal_cidr} />
        <ResourceTable
            groupName={rg.group.name}
            {base}
            workloads={rg.workloads}
            volumes={rg.volumes}
            buckets={rg.buckets}
            gateways={rg.gateways}
            {nodeNames}
            onOpenBucket={(bucket) => bucketDetailDialog?.open(bucket)}
            onChanged={rg.load}
            onError={(message) => (rg.error = message)}
        />
    {/if}
</div>
