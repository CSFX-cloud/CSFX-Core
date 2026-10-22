<script lang="ts">
    import { ChevronLeft, Search } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import CreateBucketDialog from "$lib/components/resource-group/create-bucket-dialog.svelte";
    import CreateGatewayDialog from "$lib/components/resource-group/create-gateway-dialog.svelte";
    import CreateVolumeDialog from "$lib/components/resource-group/create-volume-dialog.svelte";
    import DeployContainerDialog from "$lib/components/resource-group/deploy-container-dialog.svelte";
    import DeployVmDialog from "$lib/components/resource-group/deploy-vm-dialog.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import {
        RESOURCE_CATEGORIES,
        RESOURCE_TYPES,
        type ResourceCategory,
        type ResourceTypeKey,
    } from "$lib/components/resource-group/resource-types";

    type Filter = "all" | ResourceCategory;

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let searchText = $state("");
    let filter = $state<Filter>("all");
    let deployDialog = $state<DeployContainerDialog | null>(null);
    let vmDialog = $state<DeployVmDialog | null>(null);
    let volumeDialog = $state<CreateVolumeDialog | null>(null);
    let bucketDialog = $state<CreateBucketDialog | null>(null);
    let gatewayDialog = $state<CreateGatewayDialog | null>(null);

    const matching = $derived(
        RESOURCE_TYPES.filter((type) => `${type.label} ${type.description} ${type.equivalent}`.toLowerCase().includes(searchText.trim().toLowerCase())),
    );
    const sections = $derived(
        RESOURCE_CATEGORIES.map((category) => ({ ...category, types: matching.filter((type) => type.category === category.id) })).filter(
            (section) => section.types.length > 0 && (filter === "all" || filter === section.id),
        ),
    );
    const tabs = $derived([
        { value: "all" as Filter, label: `All ${matching.length}` },
        ...RESOURCE_CATEGORIES.map((category) => ({
            value: category.id as Filter,
            label: `${category.label} ${matching.filter((type) => type.category === category.id).length}`,
        })),
    ]);

    $effect(() => {
        breadcrumb.set(rg.group ? `${rg.group.name} / Add resource` : null);
        return () => breadcrumb.set(null);
    });

    async function finish() {
        await rg.load();
        goto(base);
    }

    async function openGateway(gatewayId: string) {
        await rg.load();
        goto(`${base}/gateway/${gatewayId}`);
    }

    function pick(key: ResourceTypeKey) {
        const openers: Record<ResourceTypeKey, () => void> = {
            "docker-container": () => deployDialog?.open(),
            "docker-compose": () => goto(`${base}/add/compose`),
            vm: () => vmDialog?.open(),
            "api-gateway": () => gatewayDialog?.open(),
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
<CreateGatewayDialog bind:this={gatewayDialog} rgId={rg.rgId} onCreated={openGateway} />

<div class="db-content">
    <section class="cx-stage" aria-label="Add resource">
        <div class="cx-stage__canvas"></div>
        <div class="ga-head">
            <div style="display: grid; gap: 10px">
                <a class="ga-back" href={base}>
                    <ChevronLeft class="cx-i" aria-hidden="true" />
                    Back to {rg.group?.name ?? "resource group"}
                </a>
                <h1 class="db-title">
                    Add a resource.<br />
                    <span style="color: var(--ink-muted)">It joins {rg.group?.name ?? "the group"}'s network{rg.group ? ` on ${rg.group.internal_cidr}` : ""}.</span>
                </h1>
            </div>
            <div class="cx-input-wrap db-search">
                <Search class="cx-i" aria-hidden="true" />
                <input class="cx-input cx-input--icon" type="search" placeholder="Search services" aria-label="Search services" bind:value={searchText} />
            </div>
        </div>
    </section>

    <div class="ga-tabs">
        <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Category">
            {#each tabs as tab (tab.value)}
                <button class="cx-tab" role="tab" type="button" aria-selected={filter === tab.value} onclick={() => (filter = tab.value)}>{tab.label}</button>
            {/each}
        </div>
    </div>

    {#if sections.length === 0}
        <EmptyState icon={Search} label="No resource types match the search" />
    {:else}
        <div style="display: grid; gap: 24px">
            {#each sections as section (section.id)}
                <section class="ga-sec" aria-labelledby="ga-{section.id}">
                    <div class="ga-sec__head"><h2 id="ga-{section.id}">/{section.label}</h2><span>{section.types.length}</span></div>
                    <div class="ga-grid">
                        {#each section.types as type (type.key)}
                            <button class="cx-card ga-card" type="button" onclick={() => pick(type.key)}>
                                <ServiceIcon name={type.service} size="lg" />
                                <span>
                                    <h3>{type.label}</h3>
                                    <p>{type.description}</p>
                                    <small>{type.equivalent}</small>
                                </span>
                            </button>
                        {/each}
                    </div>
                </section>
            {/each}
        </div>
    {/if}
</div>
