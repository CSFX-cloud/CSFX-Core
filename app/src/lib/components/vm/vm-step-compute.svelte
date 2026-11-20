<script lang="ts">
    import { onMount } from "svelte";
    import { listNodes } from "$lib/api/nodes";
    import { architectureLabel, VM_ARCHITECTURES } from "$lib/utils/vm-options";
    import type { VmDraft } from "./vm-draft.svelte";

    let { draft }: { draft: VmDraft } = $props();

    let nodeCounts = $state<Record<string, number>>({});
    let nodesKnown = $state(false);

    const offered = $derived(VM_ARCHITECTURES.filter((architecture) => !nodesKnown || (nodeCounts[architecture.id] ?? 0) > 0));

    onMount(async () => {
        const nodes = await listNodes().catch(() => null);
        if (!nodes) return;
        const counts: Record<string, number> = {};
        for (const node of nodes.filter((candidate) => candidate.status === "Online" && !candidate.cordoned)) {
            counts[node.architecture] = (counts[node.architecture] ?? 0) + 1;
        }
        nodeCounts = counts;
        nodesKnown = true;
        if (!offered.some((architecture) => architecture.id === draft.architecture)) draft.architecture = offered[0]?.id ?? "";
    });

    $effect(() => {
        if (draft.architecture === "" && offered.length > 0) draft.architecture = offered[0].id;
    });
</script>

<div class="md__fields md__fields--two">
    <div class="cx-field md__span">
        <span class="cx-label">Architecture</span>
        {#if offered.length === 0}
            <p class="cx-hint cx-hint--error">No online node can run virtual machines.</p>
        {:else}
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Architecture">
                {#each offered as architecture (architecture.id)}
                    <button
                        class="cx-tab"
                        type="button"
                        role="tab"
                        aria-selected={draft.architecture === architecture.id}
                        onclick={() => (draft.architecture = architecture.id)}
                    >
                        {architecture.label}
                    </button>
                {/each}
            </div>
            <p class="cx-hint">
                {#if nodesKnown}
                    The VM runs on a node with the same architecture. {architectureLabel(draft.architecture)}: {nodeCounts[draft.architecture] ?? 0} online node(s).
                {:else}
                    The VM runs on a node with the same architecture.
                {/if}
            </p>
        {/if}
    </div>
    <div class="cx-field">
        <label class="cx-label" for="vm-vcpus">vCPUs</label>
        <input id="vm-vcpus" type="number" min="1" step="1" class="cx-input" bind:value={draft.vcpus} />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="vm-mem-gb">Memory (GB)</label>
        <input id="vm-mem-gb" type="number" min="0.5" step="0.5" class="cx-input" bind:value={draft.memoryGb} />
    </div>
</div>
