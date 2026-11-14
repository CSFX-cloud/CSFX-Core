<script lang="ts">
    import { ExternalLink, KeyRound, Plus } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import type { Gateway } from "$lib/api/ingress";
    import {
        deleteBucket,
        deleteStack,
        deleteVolume,
        deleteWorkload,
        restartStack,
        restartWorkload,
        stopStack,
        stopWorkload,
        type Bucket,
        type Volume,
        type Workload,
    } from "$lib/api/resource-groups";
    import { deleteRegistry, type Registry } from "$lib/api/registries";
    import { deleteSite, redeploySite, type SiteSummary } from "$lib/api/sites";
    import { deleteVault, type Vault } from "$lib/api/vaults";
    import { buildResourceItems, filterResources, workloadHref, type ResourceTab } from "$lib/utils/resource-items";
    import {
        singleDeletePreview,
        stackDeletePreview,
        stopAndWait,
        volumeDeletePreview,
        workloadDeletePreview,
        displayName,
        type DeleteEntry,
        type DeletePreview,
        type GroupInventory,
    } from "$lib/utils/delete-preview";
    import BucketRow from "./bucket-row.svelte";
    import DeleteDialog from "./delete-dialog.svelte";
    import SiteRow from "./site-row.svelte";
    import StackRow from "./stack-row.svelte";
    import SimpleResourceRow from "./simple-resource-row.svelte";
    import VolumeRow from "./volume-row.svelte";
    import WorkloadRow from "./workload-row.svelte";

    const TABS: { value: ResourceTab; label: string }[] = [
        { value: "all", label: "All" },
        { value: "compute", label: "Compute" },
        { value: "containers", label: "Containers" },
        { value: "storage", label: "Storage" },
        { value: "security", label: "Security" },
    ];
    const NODE_ID_LENGTH = 8;

    type Props = {
        groupId: string;
        groupName: string;
        base: string;
        workloads: Workload[];
        volumes: Volume[];
        buckets: Bucket[];
        vaults: Vault[];
        sites: SiteSummary[];
        registries: Registry[];
        gateways: Gateway[];
        nodeNames: Map<string, string>;
        onChanged: () => Promise<void>;
        onError: (message: string) => void;
    };

    let { groupId, groupName, base, workloads, volumes, buckets, vaults, sites, registries, gateways, nodeNames, onChanged, onError }: Props = $props();

    let activeTab = $state<ResourceTab>("all");
    let expandedStacks = $state<Set<string>>(new Set());
    let deleteDialog = $state<DeleteDialog | null>(null);

    const allResources = $derived(buildResourceItems(workloads, volumes, buckets, vaults, sites, registries));
    const visible = $derived(filterResources(allResources, activeTab));

    function token(): string {
        if (!auth.token) throw new Error("Not authenticated");
        return auth.token;
    }

    async function run(action: () => Promise<unknown>, failure: string) {
        try {
            await action();
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : failure);
        }
    }

    function inventory(): GroupInventory {
        return { workloads, volumes, buckets, vaults, sites, registries, gateways };
    }

    function requestDelete(title: string, name: string, preview: DeletePreview, remove: () => Promise<unknown>) {
        deleteDialog?.open({
            title,
            name,
            requireStop: false,
            load: async () => preview,
            run: async (loaded, stopRunning) => {
                if (stopRunning) await stopAndWait(token(), groupId, loaded.running);
                await remove();
                await onChanged();
            },
        });
    }

    function requestSimpleDelete(title: string, entry: DeleteEntry, remove: () => Promise<unknown>) {
        requestDelete(title, entry.name, singleDeletePreview(entry), remove);
    }

    function toggleStack(stackId: string) {
        const next = new Set(expandedStacks);
        if (!next.delete(stackId)) next.add(stackId);
        expandedStacks = next;
    }

    function workloadIcon(workload: Workload): string {
        if (gateways.some((gateway) => gateway.workload_id === workload.id)) return "api-gateway";
        return workload.runtime_class === "vm" ? "virtual-machines" : "docker-containers";
    }

    function nodeName(agentId: string | null): string {
        if (!agentId) return "—";
        return nodeNames.get(agentId) ?? agentId.slice(0, NODE_ID_LENGTH);
    }
</script>

{#snippet workloadRow(w: Workload, indented: boolean)}
    <WorkloadRow
        {w}
        {indented}
        href={workloadHref(base, w, gateways)}
        icon={workloadIcon(w)}
        nodeName={nodeName(w.assigned_agent_id)}
        onRestart={() => run(() => restartWorkload(token(), w.id), "Failed to restart container")}
        onStop={() => run(() => stopWorkload(token(), w.id), "Failed to stop container")}
        onDelete={() => requestDelete(`Delete ${displayName(w)}`, displayName(w), workloadDeletePreview(w, inventory()), () => deleteWorkload(token(), w.id))}
    />
{/snippet}

<DeleteDialog bind:this={deleteDialog} />

{#if allResources.length === 0}
    <section class="cx-stage empty" aria-label="No resources">
        <div class="cx-stage__canvas"></div>
        <div class="empty__in">
            <span class="glow" aria-hidden="true"><Plus class="cx-i" /></span>
            <h2>Nothing runs in {groupName} yet.</h2>
            <p>Add a VM, container, volume or bucket. It joins this group's private network automatically.</p>
            <a class="cx-btn cx-btn--primary cx-btn--sm" href="{base}/add" style="margin-top: 8px">
                <Plus class="cx-i" aria-hidden="true" />
                Add resource
            </a>
        </div>
    </section>
{:else}
    <section class="cx-panel" aria-labelledby="rs-title">
        <div class="cx-panel__head" style="align-items: center">
            <div>
                <h2 class="cx-panel__title" id="rs-title">Resources</h2>
                <p class="cx-panel__sub">Everything running in {groupName}</p>
            </div>
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Filter by kind">
                {#each TABS as tab (tab.value)}
                    <button class="cx-tab" role="tab" type="button" aria-selected={activeTab === tab.value} onclick={() => (activeTab = tab.value)}>
                        {tab.label} {filterResources(allResources, tab.value).length}
                    </button>
                {/each}
            </div>
        </div>
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Name</th><th>Status</th><th>Details</th><th>Node</th><th>Created</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
                <tbody>
                    {#each visible as item (item.kind + (item.kind === "stack" ? item.data.stack_id : item.data.id))}
                        {#if item.kind === "container" || item.kind === "vm"}
                            {@render workloadRow(item.data, false)}
                        {:else if item.kind === "stack"}
                            {@const stack = item.data}
                            {@const expanded = expandedStacks.has(stack.stack_id)}
                            <StackRow
                                {stack}
                                {expanded}
                                href="{base}/stack/{stack.stack_id}"
                                nodeName={nodeName(stack.children[0]?.assigned_agent_id ?? null)}
                                onToggle={() => toggleStack(stack.stack_id)}
                                onRestart={() => run(() => restartStack(token(), stack.stack_id), "Failed to restart stack")}
                                onStop={() => run(() => stopStack(token(), stack.stack_id), "Failed to stop stack")}
                                onDelete={() => requestDelete(`Delete ${stack.stack_name}`, stack.stack_name, stackDeletePreview(stack.children, inventory()), () => deleteStack(token(), stack.stack_id))}
                            />
                            {#if expanded}
                                {#each stack.children as child (child.id)}
                                    {@render workloadRow(child, true)}
                                {/each}
                            {/if}
                        {:else if item.kind === "volume"}
                            <VolumeRow v={item.data} onDelete={() => requestDelete(`Delete ${item.data.name}`, item.data.name, volumeDeletePreview(item.data, inventory()), () => deleteVolume(token(), item.data.id))} />
                        {:else if item.kind === "vault"}
                            <SimpleResourceRow
                                name={item.data.name}
                                kind="Key Vault"
                                service="key-vault"
                                openIcon={KeyRound}
                                href="{base}/vault/{item.data.id}"
                                badgeClass={item.data.sealed ? "cx-badge--warning" : "cx-badge--success"}
                                badgeLabel={item.data.sealed ? "Sealed" : "Unsealed"}
                                detail="Soft delete {item.data.soft_delete_days} days"
                                createdAt={item.data.created_at}
                                onDelete={() => requestSimpleDelete(`Delete ${item.data.name}`, { type: "Vault", name: item.data.name, status: item.data.sealed ? "sealed" : "active" }, () => deleteVault(token(), item.data.resource_group_id, item.data.id))}
                            />
                        {:else if item.kind === "registry"}
                            <SimpleResourceRow
                                name={item.data.name}
                                kind="Container Registry"
                                service="container-registry"
                                openIcon={ExternalLink}
                                href="{base}/registry/{item.data.id}"
                                badgeClass="cx-badge--success"
                                badgeLabel="Active"
                                detail={item.data.prefix}
                                createdAt={item.data.created_at}
                                onDelete={() => requestSimpleDelete(`Delete ${item.data.name}`, { type: "Registry", name: item.data.name, status: "active" }, () => deleteRegistry(token(), item.data.resource_group_id, item.data.id))}
                            />
                        {:else if item.kind === "site"}
                            <SiteRow
                                s={item.data}
                                href="{base}/site/{item.data.id}"
                                onRedeploy={() => run(() => redeploySite(token(), item.data.resource_group_id, item.data.id), "Failed to start deployment")}
                                onDelete={() => requestSimpleDelete(`Delete ${item.data.name}`, { type: "Web app", name: item.data.name, status: item.data.latest?.status ?? "no deployment" }, () => deleteSite(token(), item.data.resource_group_id, item.data.id))}
                            />
                        {:else}
                            <BucketRow
                                b={item.data}
                                onDelete={() => requestSimpleDelete(`Delete ${item.data.name}`, { type: "Bucket", name: item.data.name, status: item.data.status }, () => deleteBucket(token(), item.data.id))}
                            />
                        {/if}
                    {/each}
                </tbody>
            </table>
        </div>
    </section>
{/if}
