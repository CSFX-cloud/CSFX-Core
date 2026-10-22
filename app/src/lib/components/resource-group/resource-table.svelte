<script lang="ts">
    import { Plus } from "@lucide/svelte";
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
    import { buildResourceItems, filterResources, workloadHref, type ResourceTab } from "$lib/utils/resource-items";
    import BucketRow from "./bucket-row.svelte";
    import StackRow from "./stack-row.svelte";
    import VolumeRow from "./volume-row.svelte";
    import WorkloadRow from "./workload-row.svelte";

    const TABS: { value: ResourceTab; label: string }[] = [
        { value: "all", label: "All" },
        { value: "compute", label: "Compute" },
        { value: "containers", label: "Containers" },
        { value: "storage", label: "Storage" },
    ];
    const NODE_ID_LENGTH = 8;

    type Props = {
        groupName: string;
        base: string;
        workloads: Workload[];
        volumes: Volume[];
        buckets: Bucket[];
        gateways: Gateway[];
        nodeNames: Map<string, string>;
        onOpenBucket: (bucket: Bucket) => void;
        onChanged: () => Promise<void>;
        onError: (message: string) => void;
    };

    let { groupName, base, workloads, volumes, buckets, gateways, nodeNames, onOpenBucket, onChanged, onError }: Props = $props();

    let activeTab = $state<ResourceTab>("all");
    let expandedStacks = $state<Set<string>>(new Set());

    const allResources = $derived(buildResourceItems(workloads, volumes, buckets));
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

    function toggleStack(stackId: string) {
        const next = new Set(expandedStacks);
        if (!next.delete(stackId)) next.add(stackId);
        expandedStacks = next;
    }

    function workloadIcon(workload: Workload): string {
        if (gateways.some((gateway) => gateway.workload_id === workload.id)) return "load-balancer";
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
        onDelete={() => run(() => deleteWorkload(token(), w.id), "Failed to delete container")}
    />
{/snippet}

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
                                onDelete={() => run(() => deleteStack(token(), stack.stack_id), "Failed to delete stack")}
                            />
                            {#if expanded}
                                {#each stack.children as child (child.id)}
                                    {@render workloadRow(child, true)}
                                {/each}
                            {/if}
                        {:else if item.kind === "volume"}
                            <VolumeRow v={item.data} onDelete={() => run(() => deleteVolume(token(), item.data.id), "Failed to delete volume")} />
                        {:else}
                            <BucketRow
                                b={item.data}
                                onOpen={() => onOpenBucket(item.data)}
                                onDelete={() => run(() => deleteBucket(token(), item.data.id), "Failed to delete bucket")}
                            />
                        {/if}
                    {/each}
                </tbody>
            </table>
        </div>
    </section>
{/if}
