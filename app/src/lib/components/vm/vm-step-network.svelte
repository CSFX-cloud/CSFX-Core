<script lang="ts">
    import { Select } from "$lib/components/ui/select";
    import { NIC_MODELS } from "$lib/utils/vm-options";
    import type { VmDraft } from "./vm-draft.svelte";

    let { draft, groupName, cidr }: { draft: VmDraft; groupName: string; cidr: string } = $props();

    const selected = $derived(NIC_MODELS.find((model) => model.id === draft.nicModel));
</script>

<div class="md__fields md__fields--two">
    <div class="cx-field md__span">
        <span class="cx-label">Network</span>
        <p class="cx-mono">{groupName} · {cidr}</p>
        <p class="cx-hint">The VM joins the private network of this resource group and gets an address from it through DHCP.</p>
    </div>
    <div class="cx-field md__span">
        <label class="cx-label" for="vm-nic">Network adapter</label>
        <Select id="vm-nic" bind:value={draft.nicModel} options={NIC_MODELS.map((model) => ({ value: model.id, label: model.label }))} />
        {#if selected}<p class="cx-hint">{selected.detail}</p>{/if}
    </div>
</div>
