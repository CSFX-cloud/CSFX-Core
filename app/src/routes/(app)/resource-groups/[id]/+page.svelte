<script lang="ts">
    import { onDestroy } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import {
        getResourceGroup,
        deleteResourceGroup,
        listResourceGroupWorkloads,
        listResourceGroupVolumes,
        listResourceGroupBuckets,
        type ResourceGroup,
        type Workload,
        type Volume,
        type Bucket,
    } from "$lib/api/resource-groups";
    import { isTransientStatus } from "$lib/utils/status.js";
    import Icon from "@iconify/svelte";
    import SearchIcon from "@lucide/svelte/icons/search";
    import { Button } from "$lib/components/ui/button/index.js";
    import { commandPalette } from "$lib/components/command-palette/command-palette-store.svelte";
    import AppearanceDialog from "$lib/components/resource-group/appearance-dialog.svelte";
    import BucketDetailDialog from "$lib/components/resource-group/bucket-detail-dialog.svelte";
    import ComposeDialog from "$lib/components/resource-group/compose-dialog.svelte";
    import ContainerDialog from "$lib/components/resource-group/container-dialog.svelte";
    import CreateBucketDialog from "$lib/components/resource-group/create-bucket-dialog.svelte";
    import CreateVolumeDialog from "$lib/components/resource-group/create-volume-dialog.svelte";
    import DeployContainerDialog from "$lib/components/resource-group/deploy-container-dialog.svelte";
    import DeployVmDialog from "$lib/components/resource-group/deploy-vm-dialog.svelte";
    import ResourcePickerDialog, { type ResourceTypeKey } from "$lib/components/resource-group/resource-picker-dialog.svelte";
    import ResourceGroupSidebar from "$lib/components/resource-group/resource-group-sidebar.svelte";
    import ResourceTable from "$lib/components/resource-group/resource-table.svelte";

    const POLL_INTERVAL_MS = 2000;

    const rgId: string = $page.params.id;

    let group = $state<ResourceGroup | null>(null);
    let workloads = $state<Workload[]>([]);
    let volumes = $state<Volume[]>([]);
    let buckets = $state<Bucket[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let downloadingVpn = $state(false);

    let appearanceDialog = $state<AppearanceDialog | null>(null);
    let volumeDialog = $state<CreateVolumeDialog | null>(null);
    let bucketDialog = $state<CreateBucketDialog | null>(null);
    let bucketDetailDialog = $state<BucketDetailDialog | null>(null);
    let resourcePickerDialog = $state<ResourcePickerDialog | null>(null);
    let deployDialog = $state<DeployContainerDialog | null>(null);
    let vmDialog = $state<DeployVmDialog | null>(null);
    let composeDialog = $state<ComposeDialog | null>(null);
    let containerDialog = $state<ContainerDialog | null>(null);

    const resourceSummaries = $derived([
        { label: "Containers", running: workloads.filter((w) => w.status === "running").length, total: workloads.length },
        { label: "Volumes", running: volumes.filter((v) => v.status === "attached" || v.status === "in_use").length, total: volumes.length },
        { label: "Buckets", running: buckets.filter((b) => b.status === "active").length, total: buckets.length },
    ]);

    const hasTransientStatus = $derived(
        workloads.some((w) => isTransientStatus(w.status)) || volumes.some((v) => isTransientStatus(v.status)),
    );

    async function load() {
        if (!auth.token) return;
        try {
            [group, workloads, volumes, buckets] = await Promise.all([
                getResourceGroup(auth.token, rgId),
                listResourceGroupWorkloads(auth.token, rgId),
                listResourceGroupVolumes(auth.token, rgId),
                listResourceGroupBuckets(auth.token, rgId),
            ]);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load";
        } finally {
            loading = false;
        }
    }

    async function reloadWorkloads() {
        if (auth.token) workloads = await listResourceGroupWorkloads(auth.token, rgId);
    }

    async function reloadVolumes() {
        if (auth.token) volumes = await listResourceGroupVolumes(auth.token, rgId);
    }

    async function reloadBuckets() {
        if (auth.token) buckets = await listResourceGroupBuckets(auth.token, rgId);
    }

    function openWorkload(workload: Workload) {
        if (workload.runtime_class === "vm") {
            goto(`/resource-groups/${rgId}/vm/${workload.id}`);
            return;
        }
        containerDialog?.open(workload);
    }

    function openResourceType(key: ResourceTypeKey) {
        const openers: Record<ResourceTypeKey, () => void> = {
            "docker-container": () => deployDialog?.open(),
            "docker-compose": () => composeDialog?.open(),
            vm: () => vmDialog?.open(),
            volume: () => volumeDialog?.open(),
            bucket: () => bucketDialog?.open(),
        };
        openers[key]();
    }

    async function downloadVpnConfig() {
        if (!auth.token) return;
        downloadingVpn = true;
        try {
            const response = await fetch(`/api/resource-groups/${rgId}/vpn-config`, {
                headers: { Authorization: `Bearer ${auth.token}` },
            });
            if (!response.ok) {
                const body = await response.json().catch(() => ({}));
                throw new Error(body.error ?? `HTTP ${response.status}`);
            }
            const url = URL.createObjectURL(await response.blob());
            const link = document.createElement("a");
            const disposition = response.headers.get("content-disposition") ?? "";
            link.href = url;
            link.download = disposition.match(/filename="([^"]+)"/)?.[1] ?? "csfx-vpn.conf";
            link.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            error = e instanceof Error ? e.message : "VPN config download failed";
        } finally {
            downloadingVpn = false;
        }
    }

    async function handleDeleteGroup() {
        if (!auth.token || !group) return;
        try {
            await deleteResourceGroup(auth.token, group.id);
            goto("/resource-groups");
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete resource group";
        }
    }

    let loadStarted = false;

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            load();
        }
    });

    let pollInterval: ReturnType<typeof setInterval> | null = null;

    $effect(() => {
        if (hasTransientStatus && !pollInterval) {
            pollInterval = setInterval(load, POLL_INTERVAL_MS);
        } else if (!hasTransientStatus && pollInterval) {
            clearInterval(pollInterval);
            pollInterval = null;
        }
    });

    onDestroy(() => {
        if (pollInterval) clearInterval(pollInterval);
    });
</script>

<ResourcePickerDialog bind:this={resourcePickerDialog} onPick={openResourceType} />
<DeployContainerDialog bind:this={deployDialog} {rgId} {volumes} onDeployed={reloadWorkloads} />
<DeployVmDialog bind:this={vmDialog} {rgId} onDeployed={reloadWorkloads} />
<ComposeDialog bind:this={composeDialog} {rgId} onDeployed={reloadWorkloads} />
<CreateVolumeDialog bind:this={volumeDialog} {rgId} onCreated={reloadVolumes} />
<CreateBucketDialog bind:this={bucketDialog} {rgId} onCreated={reloadBuckets} />
<BucketDetailDialog bind:this={bucketDetailDialog} {rgId} />
<AppearanceDialog bind:this={appearanceDialog} {group} onUpdated={(updated) => (group = updated)} />
<ContainerDialog bind:this={containerDialog} {rgId} {workloads} onChanged={reloadWorkloads} />

<div class="flex min-h-0 flex-1">
    <div class="hidden md:flex flex-col w-64 shrink-0 border-r border-border p-4">
        {#if group}
            <ResourceGroupSidebar
                {group}
                resources={resourceSummaries}
                onUpdated={(updated) => (group = updated)}
                onEditIcon={() => appearanceDialog?.open(group)}
                onDelete={handleDeleteGroup}
                onError={(message) => (error = message)}
            />
        {/if}
    </div>

    <div class="flex min-w-0 flex-1 flex-col">
        <header class="flex h-16 shrink-0 items-center gap-3 px-4">
            <Button variant="ghost" size="icon-sm" onclick={() => goto("/resource-groups")} aria-label="Back to resource groups">
                <Icon icon="mdi:arrow-left" width={16} height={16} />
            </Button>
            <div class="flex-1 flex justify-center">
                <button
                    type="button"
                    onclick={() => commandPalette.show()}
                    class="relative w-full max-w-sm text-left"
                >
                    <SearchIcon class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                    <span class="flex items-center h-9 w-full rounded-md border border-input bg-background pl-8 pr-14 text-sm text-muted-foreground hover:bg-accent/50 transition-colors">
                        Search anything
                    </span>
                    <kbd class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 rounded border border-border bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                        &#8984;K
                    </kbd>
                </button>
            </div>
            <div class="w-9"></div>
        </header>

    <div class="flex min-w-0 flex-1 flex-col gap-6 p-6">
    {#if loading}
        <p class="text-sm text-muted-foreground">Loading...</p>
    {:else if error && !group}
        <p class="text-sm text-destructive">{error}</p>
    {:else if group}
        <div class="flex items-center justify-end">
            <div class="flex items-center gap-2 shrink-0 flex-wrap justify-end">
                <Button size="sm" variant="outline" onclick={downloadVpnConfig} disabled={downloadingVpn}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
                    {downloadingVpn ? "Generating..." : "Connect VPN"}
                </Button>
                <Button size="sm" variant="outline" onclick={() => volumeDialog?.open()}>
                    Add Volume
                </Button>
                <Button size="sm" onclick={() => resourcePickerDialog?.open()}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                    Add Resource
                </Button>
            </div>
        </div>

        {#if error}
            <p class="text-xs text-destructive">{error}</p>
        {/if}

        <ResourceTable
            {workloads}
            {volumes}
            {buckets}
            onOpenWorkload={openWorkload}
            onOpenStack={(stackId) => composeDialog?.open(stackId)}
            onOpenBucket={(bucket) => bucketDetailDialog?.open(bucket)}
            onChanged={load}
            onError={(message) => (error = message)}
        />
    {/if}
    </div>
    </div>
</div>
