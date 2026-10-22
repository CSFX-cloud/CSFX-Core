<script lang="ts">
    import { onDestroy } from "svelte";
    import { Layers, Package, Play, RefreshCw, Server, Square, Trash2 } from "@lucide/svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { getNode } from "$lib/api/nodes";
    import { deleteWorkload, restartWorkload, stopWorkload } from "$lib/api/resource-groups";
    import ContainerInsightsTab from "$lib/components/resource-group/container-insights-tab.svelte";
    import ContainerLogsTab from "$lib/components/resource-group/container-logs-tab.svelte";
    import ContainerNetworkTab from "$lib/components/resource-group/container-network-tab.svelte";
    import ContainerSettingsTab from "$lib/components/resource-group/container-settings-tab.svelte";
    import ContainerShellTab from "$lib/components/resource-group/container-shell-tab.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import WorkloadHero, { type MetaItem } from "$lib/components/workload/workload-hero.svelte";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";
    import { knownWorkloadIcon } from "$lib/utils/image-icon";

    type Tab = "logs" | "shell" | "insights" | "network" | "settings";

    const POLL_INTERVAL_MS = 5000;
    const rg = useResourceGroupState();
    const workloadId = $page.params.workloadId;

    let tab = $state<Tab>("logs");
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);
    let nodeName = $state<string | null>(null);

    const container = $derived(rg.workloads.find((workload) => workload.id === workloadId) ?? null);
    const name = $derived(container?.service_name ?? container?.name ?? "");
    const stopped = $derived(container?.desired_state === "stopped" || container?.status === "stopped");
    const tabs = $derived<{ id: Tab; label: string }[]>([
        { id: "logs", label: "Logs" },
        { id: "shell", label: "Shell" },
        { id: "insights", label: "Performance" },
        { id: "network", label: "Network" },
        ...(container?.stack_id ? [] : [{ id: "settings" as Tab, label: "Settings" }]),
    ]);
    const meta = $derived<MetaItem[]>([
        { icon: Package, text: container?.image ?? "" },
        { icon: Layers, text: rg.group?.name ?? "Resource group", href: `/resource-groups/${rg.rgId}` },
        ...(nodeName ? [{ icon: Server, text: nodeName }] : []),
    ]);

    async function runAction(action: (token: string, id: string) => Promise<unknown>, failure: string) {
        if (!auth.token || !container) return false;
        actionBusy = true;
        actionError = null;
        try {
            await action(auth.token, container.id);
            await rg.load();
            return true;
        } catch (e) {
            actionError = e instanceof Error ? e.message : failure;
            return false;
        } finally {
            actionBusy = false;
        }
    }

    async function remove() {
        if (!confirm(`Delete ${name}?`)) return;
        if (await runAction(deleteWorkload, "Failed to delete container")) goto(`/resource-groups/${rg.rgId}`);
    }

    $effect(() => {
        const agentId = container?.assigned_agent_id;
        const token = auth.token;
        if (!agentId || !token) return;
        getNode(token, agentId)
            .then((node) => (nodeName = node.hostname))
            .catch(() => (nodeName = null));
    });

    $effect(() => {
        breadcrumb.set(rg.group && name ? `${rg.group.name} / ${name}` : null);
        return () => breadcrumb.set(null);
    });

    const pollInterval = setInterval(rg.load, POLL_INTERVAL_MS);

    onDestroy(() => clearInterval(pollInterval));
</script>

<div class="db-content">
    {#if rg.loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !container}
        <p class="cx-hint cx-hint--error">Container not found.</p>
    {:else}
        <WorkloadHero label="Container {name}" title={name} status={container.status} {meta}>
            {#snippet lead()}
                <ServiceIcon name="docker-containers" size="xl" brand={knownWorkloadIcon(container.image, container.runtime_class)} />
            {/snippet}
            {#snippet actions()}
                {#if stopped}
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionBusy} onclick={() => runAction(restartWorkload, "Failed to start container")}>
                        <Play class="cx-i" aria-hidden="true" />Start
                    </button>
                {:else}
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionBusy} onclick={() => runAction(stopWorkload, "Failed to stop container")}>
                        <Square class="cx-i" aria-hidden="true" />Stop
                    </button>
                {/if}
                <button class="cx-icon-btn" type="button" disabled={actionBusy} aria-label="Restart" title="Restart" onclick={() => runAction(restartWorkload, "Failed to restart container")}>
                    <RefreshCw class="cx-i" aria-hidden="true" />
                </button>
                <button class="cx-icon-btn nd-danger" type="button" disabled={actionBusy} aria-label="Delete" title="Delete" onclick={remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            {/snippet}
        </WorkloadHero>

        {#if actionError}<p class="cx-hint cx-hint--error">{actionError}</p>{/if}

        <WorkloadTabs {tabs} active={tab} label="Container sections" onselect={(id) => (tab = id)} />

        {#if tab === "logs"}
            <ContainerLogsTab workloadId={container.id} running={!stopped} />
        {:else if tab === "shell"}
            <ContainerShellTab workload={container} onstart={() => runAction(restartWorkload, "Failed to start container")} />
        {:else if tab === "insights"}
            <ContainerInsightsTab workload={container} />
        {:else if tab === "network"}
            <ContainerNetworkTab workload={container} rgId={rg.rgId} groupName={rg.group?.name ?? "the group"} cidr={rg.group?.internal_cidr ?? "—"} />
        {:else}
            <ContainerSettingsTab workload={container} onRedeployed={rg.load} />
        {/if}
    {/if}
</div>
