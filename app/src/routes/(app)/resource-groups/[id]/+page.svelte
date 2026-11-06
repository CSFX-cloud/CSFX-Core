<script lang="ts">
    import { untrack } from "svelte";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { listNodes } from "$lib/api/nodes";
    import { deleteResourceGroup } from "$lib/api/resource-groups";
    import DeleteDialog from "$lib/components/resource-group/delete-dialog.svelte";
    import AppearanceDialog from "$lib/components/resource-group/appearance-dialog.svelte";
    import GroupHero from "$lib/components/resource-group/group-hero.svelte";
    import GroupStats from "$lib/components/resource-group/group-stats.svelte";
    import ResourceTable from "$lib/components/resource-group/resource-table.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import { groupDeletePreview, loadGroupInventory, stopAndWait } from "$lib/utils/delete-preview";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let appearanceDialog = $state<AppearanceDialog | null>(null);
    let deleteDialog = $state<DeleteDialog | null>(null);
    let nodeNames = $state(new Map<string, string>());

    async function loadNodeNames() {
        if (!auth.token) return;
        const nodes = await listNodes(auth.token).catch(() => []);
        nodeNames = new Map(nodes.map((node) => [node.id, node.hostname]));
    }

    function deleteGroup() {
        const group = rg.group;
        const token = auth.token;
        if (!token || !group) return;
        deleteDialog?.open({
            title: `Delete ${group.name}`,
            name: group.name,
            requireStop: true,
            load: async () => groupDeletePreview(await loadGroupInventory(token, group.id), group.internal_cidr),
            run: async (preview, stopRunning) => {
                if (stopRunning) await stopAndWait(token, group.id, preview.running);
                await deleteResourceGroup(token, group.id);
                await goto("/resource-groups");
            },
        });
    }

    $effect(() => {
        if (auth.token) untrack(loadNodeNames);
    });

    $effect(() => {
        breadcrumb.set(rg.group?.name ?? null);
        return () => breadcrumb.set(null);
    });
</script>

<DeleteDialog bind:this={deleteDialog} />
<AppearanceDialog bind:this={appearanceDialog} group={rg.group} onUpdated={(updated) => (rg.group = updated)} />

<div class="db-content">
    {#if rg.loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !rg.group}
        <p class="cx-hint cx-hint--error">{rg.error ?? "Resource group not found"}</p>
    {:else}
        <GroupHero
            group={rg.group}
            {base}
            health={rg.health}
            onupdated={(updated) => (rg.group = updated)}
            onerror={(message) => (rg.error = message)}
            oneditappearance={() => appearanceDialog?.open(rg.group)}
            ondelete={deleteGroup}
        />

        {#if rg.error}<p class="cx-hint cx-hint--error">{rg.error}</p>{/if}

        <GroupStats workloads={rg.workloads} volumes={rg.volumes} buckets={rg.buckets} cidr={rg.group.internal_cidr} />
        <ResourceTable
            groupId={rg.group.id}
            groupName={rg.group.name}
            {base}
            workloads={rg.workloads}
            volumes={rg.volumes}
            buckets={rg.buckets}
            vaults={rg.vaults}
            sites={rg.sites}
            registries={rg.registries}
            gateways={rg.gateways}
            {nodeNames}
            onChanged={rg.load}
            onError={(message) => (rg.error = message)}
        />
    {/if}
</div>
