<script lang="ts">
    import { Plus, Trash2 } from "@lucide/svelte";
    import { Select } from "$lib/components/ui/select";
    import { NIC_MODELS } from "$lib/utils/vm-options";
    import type { VmDraft } from "./vm-draft.svelte";

    let { draft, groupName, cidr, usedIps }: { draft: VmDraft; groupName: string; cidr: string; usedIps: string[] } = $props();

    const selected = $derived(NIC_MODELS.find((model) => model.id === draft.nicModel));

    const error = $derived(draft.networkError(cidr, usedIps));
</script>

<div class="md__fields md__fields--two">
    <div class="cx-field md__span">
        <span class="cx-label">Network</span>
        <p class="cx-mono">{groupName} · {cidr}</p>
        <p class="cx-hint">Every adapter is a separate address in this network. DHCP picks the next free one, Static reserves the address you enter.</p>
    </div>
    {#each draft.nics as nic, index (index)}
        <div class="cx-field md__span">
            <span class="cx-label">{index === 0 ? "Primary address" : `Additional address ${index}`}</span>
            <div class="md__foot" style="justify-content: flex-start; align-items: center">
                <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Address mode">
                    <button class="cx-tab" type="button" role="tab" aria-selected={nic.mode === "dhcp"} onclick={() => (nic.mode = "dhcp")}>DHCP</button>
                    <button class="cx-tab" type="button" role="tab" aria-selected={nic.mode === "static"} onclick={() => (nic.mode = "static")}>Static</button>
                </div>
                {#if nic.mode === "static"}
                    <input
                        class="cx-input cx-mono"
                        style="flex: 1"
                        placeholder="e.g. {cidr.replace(/\.\d+\/\d+$/, '.10')}"
                        autocomplete="off"
                        aria-label="Static address {index + 1}"
                        aria-invalid={error !== null}
                        bind:value={nic.ip}
                    />
                {:else}
                    <span class="cx-hint" style="flex: 1">Assigned automatically</span>
                {/if}
                {#if index > 0}
                    <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Remove address {index}" title="Remove" onclick={() => draft.removeNic(index)}>
                        <Trash2 class="cx-i" aria-hidden="true" />
                    </button>
                {/if}
            </div>
        </div>
    {/each}
    <div class="cx-field md__span">
        <div class="md__foot" style="justify-content: flex-start; align-items: center">
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={!draft.canAddNic} onclick={() => draft.addNic()}>
                <Plus class="cx-i" aria-hidden="true" />
                Add address
            </button>
            <span class="cx-hint">Each additional address is a separate network adapter in the VM.</span>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
    </div>
    <div class="cx-field md__span">
        <label class="cx-label" for="vm-nic">Network adapter model</label>
        <Select id="vm-nic" bind:value={draft.nicModel} options={NIC_MODELS.map((model) => ({ value: model.id, label: model.label }))} />
        {#if selected}<p class="cx-hint">{selected.detail}</p>{/if}
    </div>
</div>
