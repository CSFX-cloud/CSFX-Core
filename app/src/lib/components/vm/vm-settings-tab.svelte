<script lang="ts">
    import { untrack, type Component } from "svelte";
    import { HardDrive, Cpu, Network, Plus, Trash2 } from "@lucide/svelte";
    import VmAddStorageDialog from "./vm-add-storage-dialog.svelte";
    import { updateWorkload, type Volume, type Workload } from "$lib/api/resource-groups";
    import { Select } from "$lib/components/ui/select";
    import { BYTES_PER_GIB, guestAddressError, MAX_EXTRA_NICS, NIC_MODELS } from "$lib/utils/vm-options";
    import { currentAddresses, initialForm, vmUpdateRequest, type VmEditForm } from "./vm-edit";

    type SectionId = "compute" | "storage" | "network";
    type Props = { workload: Workload; rgId: string; volumes: Volume[]; cidr: string; usedIps: string[]; onSaved: () => Promise<void> };

    const SECTIONS: { id: SectionId; label: string; icon: Component }[] = [
        { id: "compute", label: "CPU and memory", icon: Cpu },
        { id: "storage", label: "Disk", icon: HardDrive },
        { id: "network", label: "Network", icon: Network },
    ];

    let { workload, rgId, volumes, cidr, usedIps, onSaved }: Props = $props();

    let section = $state<SectionId>("compute");
    let storageDialog = $state<VmAddStorageDialog | null>(null);
    let form = $state<VmEditForm>(untrack(() => initialForm(workload)));
    let busy = $state(false);
    let error = $state<string | null>(null);

    const stopped = $derived(workload.desired_state === "stopped" || workload.status === "stopped");
    const request = $derived(vmUpdateRequest(workload, form));
    const changed = $derived(Object.keys(request).length > 0);
    const currentDiskGb = $derived(workload.disk_bytes / BYTES_PER_GIB);
    const ownIps = $derived(currentAddresses(workload));
    const taken = $derived(usedIps.filter((ip) => !ownIps.includes(ip)));
    const filled = $derived(form.addresses.map((ip) => ip.trim()).filter((ip) => ip !== ""));
    const addressError = $derived(guestAddressError(cidr, taken, filled));
    const computeValid = $derived(Number.isInteger(form.vcpus) && form.vcpus >= 1 && form.memoryGb >= 0.5);
    const valid = $derived(changed && computeValid && form.diskGb >= currentDiskGb && addressError === null);
    const attached = $derived(volumes.filter((volume) => form.volumeIds.includes(volume.id)));
    const free = $derived(volumes.filter((volume) => volume.attached_to_workload === null && !form.volumeIds.includes(volume.id)));
    const selectedModel = $derived(NIC_MODELS.find((model) => model.id === form.nicModel));

    function toggleVolume(volumeId: string) {
        form.volumeIds = form.volumeIds.includes(volumeId) ? form.volumeIds.filter((id) => id !== volumeId) : [...form.volumeIds, volumeId];
    }

    function reset() {
        form = initialForm(workload);
        error = null;
    }

    async function save() {
        if (!valid) return;
        busy = true;
        error = null;
        try {
            await updateWorkload(workload.id, request);
            await onSaved();
            reset();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to save VM settings";
        } finally {
            busy = false;
        }
    }
</script>

<div class="st-main vm-settings">
    <nav class="st-nav" aria-label="VM settings">
        {#each SECTIONS as item (item.id)}
            <button class="cx-side__item" type="button" aria-current={section === item.id ? "page" : undefined} onclick={() => (section = item.id)}>
                <item.icon class="cx-i" aria-hidden="true" />{item.label}
            </button>
        {/each}
    </nav>
    <form
        class="st-body"
        onsubmit={(event) => {
            event.preventDefault();
            save();
        }}
    >
        <section class="cx-panel" aria-labelledby="vms-title">
            <div class="cx-panel__head"><h2 class="cx-panel__title" id="vms-title">{SECTIONS.find((item) => item.id === section)?.label}</h2></div>
            <div class="md__fields md__fields--two">
                {#if section === "compute"}
                    <div class="cx-field">
                        <label class="cx-label" for="vms-vcpus">vCPUs</label>
                        <input id="vms-vcpus" type="number" min="1" step="1" class="cx-input" bind:value={form.vcpus} />
                    </div>
                    <div class="cx-field">
                        <label class="cx-label" for="vms-mem">Memory (GB)</label>
                        <input id="vms-mem" type="number" min="0.5" step="0.5" class="cx-input" bind:value={form.memoryGb} />
                    </div>
                {:else if section === "storage"}
                    <div class="cx-field md__span">
                        <label class="cx-label" for="vms-disk">Disk (GB)</label>
                        <input id="vms-disk" type="number" min={currentDiskGb} step="1" class="cx-input" bind:value={form.diskGb} />
                        <p class="cx-hint">The disk can only grow. Extend the partition inside the guest to use the new space.</p>
                    </div>
                    <div class="cx-field md__span">
                        <span class="cx-label">Block storage</span>
                        {#each attached as volume (volume.id)}
                            <div class="md__foot" style="justify-content: flex-start; align-items: center">
                                <span style="flex: 1">{volume.name} <span class="cx-hint">{volume.size_gb} GB, {volume.pool}</span></span>
                                <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Detach {volume.name}" title="Detach" onclick={() => toggleVolume(volume.id)}>
                                    <Trash2 class="cx-i" aria-hidden="true" />
                                </button>
                            </div>
                        {/each}
                        <div class="md__foot" style="justify-content: flex-start; align-items: center">
                            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => storageDialog?.open()}>
                                <Plus class="cx-i" aria-hidden="true" />
                                Add block storage
                            </button>
                            <span class="cx-hint">Attached volumes show up in the guest as additional VirtIO block devices (vdb, vdc, and so on).</span>
                        </div>
                    </div>
                {:else}
                    {#each form.addresses as _, index (index)}
                        <div class="cx-field md__span">
                            <label class="cx-label" for="vms-ip-{index}">{index === 0 ? "Primary address" : `Additional address ${index}`}</label>
                            <div class="md__foot" style="justify-content: flex-start; align-items: center">
                                <input
                                    id="vms-ip-{index}"
                                    class="cx-input cx-mono"
                                    style="flex: 1"
                                    placeholder="Automatic"
                                    autocomplete="off"
                                    aria-invalid={addressError !== null}
                                    bind:value={form.addresses[index]}
                                />
                                {#if index > 0}
                                    <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Remove address {index}" title="Remove" onclick={() => (form.addresses = form.addresses.filter((_, position) => position !== index))}>
                                        <Trash2 class="cx-i" aria-hidden="true" />
                                    </button>
                                {/if}
                            </div>
                        </div>
                    {/each}
                    <div class="cx-field md__span">
                        <div class="md__foot" style="justify-content: flex-start; align-items: center">
                            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={form.addresses.length > MAX_EXTRA_NICS} onclick={() => (form.addresses = [...form.addresses, ""])}>
                                <Plus class="cx-i" aria-hidden="true" />
                                Add address
                            </button>
                            <span class="cx-hint">Leave an address empty to pick the next free one in {cidr}.</span>
                        </div>
                        {#if addressError}<p class="cx-hint cx-hint--error">{addressError}</p>{/if}
                    </div>
                    <div class="cx-field md__span">
                        <label class="cx-label" for="vms-nic">Network adapter model</label>
                        <Select id="vms-nic" bind:value={form.nicModel} options={NIC_MODELS.map((model) => ({ value: model.id, label: model.label }))} />
                        {#if selectedModel}<p class="cx-hint">{selectedModel.detail}</p>{/if}
                    </div>
                {/if}
            </div>
        </section>
        <div class="vm-settings__bar">
            <span class="cx-hint vm-settings__note">{stopped ? "Changes apply the next time the VM starts." : "Saving restarts the VM to apply the changes."}</span>
            {#if error}<span class="cx-hint cx-hint--error">{error}</span>{/if}
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" disabled={busy || !changed} onclick={reset}>Discard</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={busy || !valid} aria-busy={busy}>{busy ? "Saving" : "Save"}</button>
        </div>
    </form>
</div>

<VmAddStorageDialog bind:this={storageDialog} {rgId} {free} onPick={toggleVolume} onVolumeCreated={onSaved} />
