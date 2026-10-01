<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
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
    import {
        buildResourceItems,
        filterResources,
        type ResourceTab,
    } from "$lib/utils/resource-items";
    import BucketRow from "./bucket-row.svelte";
    import StackRow from "./stack-row.svelte";
    import VolumeRow from "./volume-row.svelte";
    import WorkloadRow from "./workload-row.svelte";

    let {
        workloads,
        volumes,
        buckets,
        onOpenWorkload,
        onOpenStack,
        onOpenBucket,
        onChanged,
        onError,
    }: {
        workloads: Workload[];
        volumes: Volume[];
        buckets: Bucket[];
        onOpenWorkload: (workload: Workload) => void;
        onOpenStack: (stackId: string) => void;
        onOpenBucket: (bucket: Bucket) => void;
        onChanged: () => Promise<void>;
        onError: (message: string) => void;
    } = $props();

    let activeTab = $state<ResourceTab>("all");
    let filterText = $state("");
    let expandedStacks = $state<Set<string>>(new Set());

    const allResources = $derived(buildResourceItems(workloads, volumes, buckets));
    const filteredResources = $derived(filterResources(allResources, activeTab, filterText));

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
</script>

{#snippet workloadRow(w: Workload, indented: boolean)}
    <WorkloadRow
        {w}
        {indented}
        onOpen={() => onOpenWorkload(w)}
        onRestart={() => run(() => restartWorkload(token(), w.id), "Failed to restart container")}
        onStop={() => run(() => stopWorkload(token(), w.id), "Failed to stop container")}
        onDelete={() => run(() => deleteWorkload(token(), w.id), "Failed to delete container")}
    />
{/snippet}

<div class="border rounded-lg overflow-hidden">
    <div class="px-4 py-3 border-b flex items-center justify-between gap-4 flex-wrap">
        <div class="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-muted">
            {#each [["all", `All ${allResources.length}`], ["container", `Container ${workloads.filter((w) => w.runtime_class !== "vm").length}`], ["vm", `VM ${workloads.filter((w) => w.runtime_class === "vm").length}`], ["volume", `Volume ${volumes.length}`], ["bucket", `Bucket ${buckets.length}`]] as [tab, label]}
                <button
                    class="px-3 py-1 rounded-md text-sm font-medium transition-all duration-200 {activeTab === tab ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                    onclick={() => (activeTab = tab as ResourceTab)}
                >
                    {label}
                </button>
            {/each}
        </div>
        <div class="flex items-center gap-2 border rounded px-3 py-1.5 text-sm min-w-[180px]">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-muted-foreground shrink-0"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
            <input class="bg-transparent outline-none flex-1 text-sm" placeholder="Filter resources..." bind:value={filterText} />
        </div>
    </div>

    <table class="w-full text-sm">
        <thead>
            <tr class="bg-muted/30 border-b">
                <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Resource</th>
                <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Kind</th>
                <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Host / Size</th>
                <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Load</th>
                <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Status</th>
                <th class="px-4 py-2.5"></th>
            </tr>
        </thead>
        <tbody>
                    {#if filteredResources.length === 0}
                        <tr>
                            <td colspan="6" class="px-4 py-10 text-center text-muted-foreground text-sm">
                                {allResources.length === 0 ? "No resources yet. Deploy a container or create a volume." : "No resources match filter."}
                            </td>
                        </tr>
                    {:else}
                        {#each filteredResources as item (item.kind + (item.kind === "stack" ? item.data.stack_id : item.data.id))}
                            {#if item.kind === "container" || item.kind === "vm"}
                                {@render workloadRow(item.data, false)}
                            {:else if item.kind === "stack"}
                                {@const stack = item.data}
                                {@const expanded = expandedStacks.has(stack.stack_id)}
                                <StackRow
                                    {stack}
                                    {expanded}
                                    onOpen={() => onOpenStack(stack.stack_id)}
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
                                <VolumeRow
                                    v={item.data}
                                    onDelete={() => run(() => deleteVolume(token(), item.data.id), "Failed to delete volume")}
                                />
                            {:else if item.kind === "bucket"}
                                <BucketRow
                                    b={item.data}
                                    onOpen={() => onOpenBucket(item.data)}
                                    onDelete={() => run(() => deleteBucket(token(), item.data.id), "Failed to delete bucket")}
                                />
                            {/if}
                        {/each}
                    {/if}
                </tbody>
            </table>
        </div>
