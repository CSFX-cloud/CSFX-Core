<script lang="ts">
    import type { Volume } from "$lib/api/resource-groups";
    import { architectureLabel, NIC_MODELS } from "$lib/utils/vm-options";
    import ProgressRing from "./progress-ring.svelte";
    import type { VmDraft } from "./vm-draft.svelte";

    let { draft, volumes, cidr }: { draft: VmDraft; volumes: Volume[]; cidr: string } = $props();

    const attached = $derived(volumes.filter((volume) => draft.volumeIds.includes(volume.id)));
    const nicLabel = $derived(NIC_MODELS.find((model) => model.id === draft.nicModel)?.label ?? draft.nicModel);
    const rows = $derived([
        ["Name", draft.name.trim()],
        ["Image", draft.imageLabel],
        ["Architecture", architectureLabel(draft.architecture)],
        ["Compute", `${draft.vcpus} vCPU, ${draft.memoryGb} GB memory`],
        ["Boot disk", `${draft.diskGb} GB`],
        ["Block storage", attached.length > 0 ? attached.map((volume) => volume.name).join(", ") : "None"],
        ["Network", `${cidr}, ${nicLabel}`],
    ]);
</script>

<dl class="md__fields" style="display: grid; gap: 10px; margin: 0">
    {#each rows as [label, value] (label)}
        <div style="display: flex; gap: 16px; justify-content: space-between">
            <dt class="cx-label">{label}</dt>
            <dd class="cx-mono" style="margin: 0">{value}</dd>
        </div>
    {/each}
</dl>

{#if draft.deploying && draft.isoMode === "upload"}
    <div class="md__foot" style="justify-content: flex-start; align-items: center">
        <ProgressRing fraction={draft.uploadProgress} />
        <span class="cx-hint">Uploading {draft.imageLabel}</span>
        <span class="cx-hint cx-mono">{Math.round(draft.uploadProgress * 100)}%</span>
    </div>
{/if}
