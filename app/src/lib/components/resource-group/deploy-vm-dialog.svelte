<script lang="ts">
    import {
        createWorkload,
        ensureIsoBucket,
        findIsoBucket,
        uploadIso,
        listBucketObjects,
        presignObjectDownload,
        type ObjectEntry,
    } from "$lib/api/resource-groups";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId, onDeployed }: { rgId: string; onDeployed: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let vmFormName = $state("");
    let vmFormVcpus = $state("2");
    let vmFormMemoryGb = $state("2");
    let vmFormDiskGb = $state("20");
    let vmFormAdvanced = $state(false);
    let vmFormCpuMillicores = $state("2000");
    let vmFormMemoryMb = $state("2048");
    let vmFormDiskMb = $state("20480");

    let vmEffectiveCpuMillicores = $derived(
        vmFormAdvanced ? parseInt(vmFormCpuMillicores) || 0 : (parseInt(vmFormVcpus) || 0) * 1000
    );
    let vmEffectiveMemoryMb = $derived(
        vmFormAdvanced ? parseInt(vmFormMemoryMb) || 0 : (parseInt(vmFormMemoryGb) || 0) * 1024
    );
    let vmEffectiveDiskMb = $derived(
        vmFormAdvanced ? parseInt(vmFormDiskMb) || 0 : (parseInt(vmFormDiskGb) || 0) * 1024
    );
    let vmFormIsoFile = $state<File | null>(null);
    let vmIsoMode = $state<"existing" | "upload">("upload");
    let vmExistingIsos = $state<ObjectEntry[]>([]);
    let vmExistingIsosLoading = $state(false);
    let vmSelectedIsoKey = $state("");
    let vmIsoBucketId = $state<string | null>(null);
    let vmDeploying = $state(false);
    let vmUploadProgress = $state(0);
    let vmError = $state<string | null>(null);

    export function open() {
        dialog?.open();
        loadExistingIsos();
    }

    async function loadExistingIsos() {
        vmExistingIsosLoading = true;
        try {
            const bucket = await findIsoBucket(rgId);
            vmIsoBucketId = bucket?.id ?? null;
            vmExistingIsos = bucket ? (await listBucketObjects(bucket.id, "")).objects : [];
            if (vmExistingIsos.length > 0) {
                vmIsoMode = "existing";
                vmSelectedIsoKey = vmExistingIsos[0].key;
            } else {
                vmIsoMode = "upload";
            }
        } catch (e) {
            vmError = e instanceof Error ? e.message : "Failed to load existing isos";
        } finally {
            vmExistingIsosLoading = false;
        }
    }

    function isoFileName(key: string): string {
        const parts = key.split("-");
        return parts.length > 5 ? parts.slice(5).join("-") : key;
    }

    function resetVmForm() {
        vmFormName = "";
        vmFormVcpus = "2";
        vmFormMemoryGb = "2";
        vmFormDiskGb = "20";
        vmFormAdvanced = false;
        vmFormCpuMillicores = "2000";
        vmFormMemoryMb = "2048";
        vmFormDiskMb = "20480";
        vmFormIsoFile = null;
        vmIsoMode = "upload";
        vmExistingIsos = [];
        vmSelectedIsoKey = "";
        vmIsoBucketId = null;
        vmUploadProgress = 0;
        vmError = null;
    }

    function handleVmIsoFileChange(event: Event) {
        const input = event.target as HTMLInputElement;
        vmFormIsoFile = input.files?.[0] ?? null;
    }

    async function handleDeployVm() {
        if (!vmFormName) return;
        if (vmIsoMode === "upload" && !vmFormIsoFile) return;
        if (vmIsoMode === "existing" && !vmSelectedIsoKey) return;
        vmDeploying = true;
        vmUploadProgress = 0;
        vmError = null;
        try {
            let isoUrl: string;
            if (vmIsoMode === "existing" && vmIsoBucketId) {
                const presigned = await presignObjectDownload(vmIsoBucketId, vmSelectedIsoKey);
                isoUrl = presigned.url;
            } else {
                const bucket = await ensureIsoBucket(rgId);
                isoUrl = await uploadIso(bucket.id, vmFormIsoFile!, (fraction) => {
                    vmUploadProgress = fraction;
                });
            }

            await createWorkload({
                name: vmFormName,
                image: isoUrl,
                cpu_millicores: vmEffectiveCpuMillicores,
                memory_bytes: vmEffectiveMemoryMb * 1024 * 1024,
                disk_bytes: vmEffectiveDiskMb * 1024 * 1024,
                env_vars: null,
                ports: null,
                volume_mounts: null,
                resource_group_id: rgId,
                runtime_class: "vm",
            });
            dialog?.close();
            resetVmForm();
            await onDeployed();
        } catch (e) {
            vmError = e instanceof Error ? e.message : "Failed to deploy vm";
        } finally {
            vmDeploying = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Deploy Virtual Machine" size="md" onclose={resetVmForm}>
    <div class="md__fields">
        <div class="cx-field">
            <label class="cx-label" for="vm-name">Name</label>
            <input id="vm-name" class="cx-input" placeholder="my-vm" bind:value={vmFormName} />
        </div>
        <div class="cx-field">
            <div class="md__foot" style="justify-content: space-between; align-items: center">
                <span class="cx-label">ISO Image</span>
                {#if vmExistingIsos.length > 0}
                    <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="ISO source">
                        <button class="cx-tab" role="tab" type="button" aria-selected={vmIsoMode === "existing"} onclick={() => (vmIsoMode = "existing")}>Existing</button>
                        <button class="cx-tab" role="tab" type="button" aria-selected={vmIsoMode === "upload"} onclick={() => (vmIsoMode = "upload")}>Upload new</button>
                    </div>
                {/if}
            </div>
            {#if vmExistingIsosLoading}
                <p class="cx-hint">Loading isos</p>
            {:else if vmIsoMode === "existing"}
                <select id="vm-iso-existing" class="cx-input" bind:value={vmSelectedIsoKey}>
                    {#each vmExistingIsos as obj (obj.key)}
                        <option value={obj.key}>{isoFileName(obj.key)}</option>
                    {/each}
                </select>
            {:else}
                <input id="vm-iso" type="file" accept=".iso" class="cx-input" onchange={handleVmIsoFileChange} />
                {#if vmDeploying && vmFormIsoFile}
                    <progress value={vmUploadProgress} max="1" style="width: 100%"></progress>
                {/if}
            {/if}
        </div>
        {#if vmFormAdvanced}
        <div class="cx-field">
        <label class="cx-label" for="vm-cpu">CPU (millicores)</label>
        <input id="vm-cpu" type="number" class="cx-input" placeholder="2000" bind:value={vmFormCpuMillicores} />
        </div>
        <div class="cx-field">
        <label class="cx-label" for="vm-mem">Memory (MB)</label>
        <input id="vm-mem" type="number" class="cx-input" placeholder="2048" bind:value={vmFormMemoryMb} />
        </div>
        <div class="cx-field">
        <label class="cx-label" for="vm-disk">Disk (MB)</label>
        <input id="vm-disk" type="number" class="cx-input" placeholder="20480" bind:value={vmFormDiskMb} />
        </div>
        {:else}
        <div class="cx-field">
        <label class="cx-label" for="vm-vcpus">vCPUs</label>
        <input id="vm-vcpus" type="number" min="1" class="cx-input" placeholder="2" bind:value={vmFormVcpus} />
        </div>
        <div class="cx-field">
        <label class="cx-label" for="vm-mem-gb">Memory (GB)</label>
        <input id="vm-mem-gb" type="number" min="1" class="cx-input" placeholder="2" bind:value={vmFormMemoryGb} />
        </div>
        <div class="cx-field">
        <label class="cx-label" for="vm-disk-gb">Disk (GB)</label>
        <input id="vm-disk-gb" type="number" min="1" class="cx-input" placeholder="20" bind:value={vmFormDiskGb} />
        </div>
        {/if}
        <div>
            <button type="button" class="cx-btn cx-btn--ghost cx-btn--sm" onclick={() => (vmFormAdvanced = !vmFormAdvanced)}>
                {vmFormAdvanced ? "Use simple units" : "Advanced (millicores / MB)"}
            </button>
        </div>
        <p class="cx-hint">The ISO boots on first start. Open the console after deploying to complete the installation.</p>
        {#if vmError}<p class="cx-hint cx-hint--error">{vmError}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button
                class="cx-btn cx-btn--primary cx-btn--sm"
                type="button"
                onclick={handleDeployVm}
                disabled={vmDeploying || !vmFormName || (vmIsoMode === "upload" ? !vmFormIsoFile : !vmSelectedIsoKey)}
                aria-busy={vmDeploying}
            >
                {vmDeploying ? "Deploying" : "Deploy"}
            </button>
        </div>
    </div>
</ModalDialog>
