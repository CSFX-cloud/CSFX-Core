<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import {
        createWorkload,
        ensureIsoBucket,
        uploadIso,
        listBucketObjects,
        presignObjectDownload,
        type ObjectEntry,
    } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import Icon from "@iconify/svelte";

    let { rgId, onDeployed }: { rgId: string; onDeployed: () => Promise<void> } = $props();

    let dialog = $state<HTMLDialogElement | null>(null);
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
        dialog?.showModal();
        loadExistingIsos();
    }

    async function loadExistingIsos() {
        if (!auth.token) return;
        vmExistingIsosLoading = true;
        try {
            const bucket = await ensureIsoBucket(auth.token, rgId);
            vmIsoBucketId = bucket.id;
            const result = await listBucketObjects(auth.token, bucket.id, "");
            vmExistingIsos = result.objects;
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
        if (!auth.token || !vmFormName) return;
        if (vmIsoMode === "upload" && !vmFormIsoFile) return;
        if (vmIsoMode === "existing" && !vmSelectedIsoKey) return;
        vmDeploying = true;
        vmUploadProgress = 0;
        vmError = null;
        try {
            let isoUrl: string;
            if (vmIsoMode === "existing" && vmIsoBucketId) {
                const presigned = await presignObjectDownload(auth.token, vmIsoBucketId, vmSelectedIsoKey);
                isoUrl = presigned.url;
            } else {
                const bucket = await ensureIsoBucket(auth.token, rgId);
                isoUrl = await uploadIso(auth.token, bucket.id, vmFormIsoFile!, (fraction) => {
                    vmUploadProgress = fraction;
                });
            }

            await createWorkload(auth.token, {
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

<dialog
    bind:this={dialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-lg rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => resetVmForm()}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Deploy Virtual Machine</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => dialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div class="flex flex-col gap-1 sm:col-span-2">
                <label class="text-xs text-muted-foreground" for="vm-name">Name</label>
                <input id="vm-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="my-vm" bind:value={vmFormName} />
            </div>
            <div class="flex flex-col gap-2 sm:col-span-2">
                <div class="flex items-center justify-between">
                    <span class="text-xs text-muted-foreground">ISO Image</span>
                    {#if vmExistingIsos.length > 0}
                        <div class="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-muted">
                            <button
                                type="button"
                                class="px-2 py-0.5 rounded text-xs font-medium transition-colors {vmIsoMode === 'existing' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                                onclick={() => (vmIsoMode = "existing")}
                            >
                                Existing
                            </button>
                            <button
                                type="button"
                                class="px-2 py-0.5 rounded text-xs font-medium transition-colors {vmIsoMode === 'upload' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                                onclick={() => (vmIsoMode = "upload")}
                            >
                                Upload new
                            </button>
                        </div>
                    {/if}
                </div>
                {#if vmExistingIsosLoading}
                    <p class="text-xs text-muted-foreground">Loading isos...</p>
                {:else if vmIsoMode === "existing"}
                    <select
                        id="vm-iso-existing"
                        class="border rounded px-3 py-1.5 text-sm bg-background"
                        bind:value={vmSelectedIsoKey}
                    >
                        {#each vmExistingIsos as obj (obj.key)}
                            <option value={obj.key}>{isoFileName(obj.key)}</option>
                        {/each}
                    </select>
                {:else}
                    <input
                        id="vm-iso"
                        type="file"
                        accept=".iso"
                        class="border rounded px-3 py-1.5 text-sm bg-background file:mr-3 file:rounded file:border-0 file:bg-muted file:px-2 file:py-1 file:text-xs"
                        onchange={handleVmIsoFileChange}
                    />
                    {#if vmDeploying && vmFormIsoFile}
                        <div class="h-1.5 rounded-full bg-muted overflow-hidden mt-1">
                            <div class="h-full bg-primary transition-all" style="width: {Math.round(vmUploadProgress * 100)}%"></div>
                        </div>
                    {/if}
                {/if}
            </div>
            {#if vmFormAdvanced}
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-cpu">CPU (millicores)</label>
                    <input id="vm-cpu" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2000" bind:value={vmFormCpuMillicores} />
                </div>
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-mem">Memory (MB)</label>
                    <input id="vm-mem" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2048" bind:value={vmFormMemoryMb} />
                </div>
                <div class="flex flex-col gap-1 sm:col-span-2">
                    <label class="text-xs text-muted-foreground" for="vm-disk">Disk (MB)</label>
                    <input id="vm-disk" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="20480" bind:value={vmFormDiskMb} />
                </div>
            {:else}
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-vcpus">vCPUs</label>
                    <input id="vm-vcpus" type="number" min="1" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2" bind:value={vmFormVcpus} />
                </div>
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-mem-gb">Memory (GB)</label>
                    <input id="vm-mem-gb" type="number" min="1" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2" bind:value={vmFormMemoryGb} />
                </div>
                <div class="flex flex-col gap-1 sm:col-span-2">
                    <label class="text-xs text-muted-foreground" for="vm-disk-gb">Disk (GB)</label>
                    <input id="vm-disk-gb" type="number" min="1" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="20" bind:value={vmFormDiskGb} />
                </div>
            {/if}
            <div class="sm:col-span-2">
                <button
                    type="button"
                    class="text-xs text-muted-foreground hover:text-foreground transition-colors underline underline-offset-2"
                    onclick={() => (vmFormAdvanced = !vmFormAdvanced)}
                >
                    {vmFormAdvanced ? "Use simple units" : "Advanced (millicores / MB)"}
                </button>
            </div>
        </div>
        <p class="text-xs text-muted-foreground">
            The ISO boots on first start. Open the console after deploying to complete the installation.
        </p>
        {#if vmError}
            <p class="text-xs text-destructive">{vmError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
            <Button
                size="sm"
                onclick={handleDeployVm}
                disabled={vmDeploying ||
                    !vmFormName ||
                    (vmIsoMode === "upload" ? !vmFormIsoFile : !vmSelectedIsoKey)}
            >
                {vmDeploying ? "Deploying..." : "Deploy"}
            </Button>
        </div>
    </div>
</dialog>
