<script lang="ts">
    import Icon from "@iconify/svelte";
    import { goto } from "$app/navigation";
    import CreateBucketDialog from "$lib/components/resource-group/create-bucket-dialog.svelte";
    import CreateVolumeDialog from "$lib/components/resource-group/create-volume-dialog.svelte";
    import DeployContainerDialog from "$lib/components/resource-group/deploy-container-dialog.svelte";
    import DeployVmDialog from "$lib/components/resource-group/deploy-vm-dialog.svelte";
    import RgTopbar from "$lib/components/resource-group/rg-topbar.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { RESOURCE_TYPES, type ResourceTypeKey } from "$lib/components/resource-group/resource-types";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let searchText = $state("");
    let deployDialog = $state<DeployContainerDialog | null>(null);
    let vmDialog = $state<DeployVmDialog | null>(null);
    let volumeDialog = $state<CreateVolumeDialog | null>(null);
    let bucketDialog = $state<CreateBucketDialog | null>(null);

    const filteredTypes = $derived(
        RESOURCE_TYPES.filter((type) => `${type.label} ${type.description}`.toLowerCase().includes(searchText.toLowerCase())),
    );

    async function finish() {
        await rg.load();
        goto(base);
    }

    function pick(key: ResourceTypeKey) {
        const openers: Record<ResourceTypeKey, () => void> = {
            "docker-container": () => deployDialog?.open(),
            "docker-compose": () => goto(`${base}/add/compose`),
            vm: () => vmDialog?.open(),
            volume: () => volumeDialog?.open(),
            bucket: () => bucketDialog?.open(),
        };
        openers[key]();
    }
</script>

<DeployContainerDialog bind:this={deployDialog} rgId={rg.rgId} volumes={rg.volumes} onDeployed={finish} />
<DeployVmDialog bind:this={vmDialog} rgId={rg.rgId} onDeployed={finish} />
<CreateVolumeDialog bind:this={volumeDialog} rgId={rg.rgId} onCreated={finish} />
<CreateBucketDialog bind:this={bucketDialog} rgId={rg.rgId} onCreated={finish} />

<RgTopbar backHref={base} backLabel="Back to resource group" />

<div class="flex min-w-0 flex-1 flex-col gap-6 p-6">
    <div>
        <h1 class="text-xl font-semibold tracking-tight">Add Resource</h1>
        <p class="text-sm text-muted-foreground mt-0.5">Choose what to deploy into this resource group</p>
    </div>

    <div class="flex items-center gap-2 border rounded px-3 py-1.5 text-sm max-w-xs w-full">
        <Icon icon="mdi:magnify" width={14} height={14} class="text-muted-foreground shrink-0" />
        <input class="bg-transparent outline-none flex-1 text-sm" placeholder="Search resource types..." bind:value={searchText} />
    </div>

    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
        {#each filteredTypes as type (type.key)}
            <button
                class="flex flex-col gap-3 border rounded-lg p-4 hover:bg-muted/30 transition-colors text-left"
                onclick={() => pick(type.key)}
            >
                <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-muted/50">
                    <Icon icon={type.icon} width={20} height={20} />
                </div>
                <div class="min-w-0">
                    <p class="font-medium text-sm">{type.label}</p>
                    <p class="text-xs text-muted-foreground mt-1">{type.description}</p>
                </div>
            </button>
        {:else}
            <p class="text-sm text-muted-foreground col-span-full text-center py-8">No resource types match search.</p>
        {/each}
    </div>
</div>
