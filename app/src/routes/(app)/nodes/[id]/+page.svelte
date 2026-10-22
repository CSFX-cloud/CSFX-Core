<script lang="ts">
    import { untrack } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import {
        clearMaintenance as clearNodeMaintenance,
        listEvents,
        resolveEvent,
        setMaintenance,
        type AlertEvent,
    } from "$lib/api/events";
    import {
        cordonNode,
        drainNode,
        getNode,
        getNodeDisks,
        powerOffNode,
        rebootNode,
        removeNode,
        uncordonNode,
        type DiskInfo,
        type Node,
    } from "$lib/api/nodes";
    import { listWorkloads, type Workload } from "$lib/api/resource-groups";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import NodeAlertsTab from "$lib/components/nodes/detail/node-alerts-tab.svelte";
    import NodeDetailHeader from "$lib/components/nodes/detail/node-detail-header.svelte";
    import NodeHardwareTab from "$lib/components/nodes/detail/node-hardware-tab.svelte";
    import { NodeLiveMetrics } from "$lib/components/nodes/detail/node-live.svelte";
    import NodeMaintenanceDialog from "$lib/components/nodes/detail/node-maintenance-dialog.svelte";
    import NodeNetworkTab from "$lib/components/nodes/detail/node-network-tab.svelte";
    import NodeStorageTab from "$lib/components/nodes/detail/node-storage-tab.svelte";
    import NodeSummaryTab from "$lib/components/nodes/detail/node-summary-tab.svelte";
    import NodeTasksTab from "$lib/components/nodes/detail/node-tasks-tab.svelte";
    import NodeWorkloadsTab from "$lib/components/nodes/detail/node-workloads-tab.svelte";
    import { NODE_TABS, type NodeActions, type NodeTab } from "$lib/components/nodes/detail/types";
    import { maintenanceLabel } from "$lib/utils/nodes";

    const nodeId: string = $page.params.id ?? "";
    const MAINTENANCE_TICK_MS = 30000;
    const live = new NodeLiveMetrics(nodeId);

    const requestedTab = $page.url.searchParams.get("tab");
    let activeTab = $state<NodeTab>(NODE_TABS.find((tab) => tab.id === requestedTab)?.id ?? "summary");

    let node = $state<Node | null>(null);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let actionBusy = $state(false);
    let actionError = $state<string | null>(null);

    let alerts = $state<AlertEvent[]>([]);
    let alertsLoading = $state(true);
    let alertsError = $state<string | null>(null);

    let workloads = $state<Workload[]>([]);
    let workloadsLoading = $state(true);
    let workloadsError = $state<string | null>(null);

    let disks = $state<DiskInfo[]>([]);
    let disksLoading = $state(false);
    let disksError = $state<string | null>(null);
    let disksLoaded = false;

    let maintenanceDialog = $state<ReturnType<typeof NodeMaintenanceDialog> | null>(null);
    let maintenanceError = $state<string | null>(null);
    let now = $state(Date.now());

    const openAlerts = $derived(alerts.filter((alert) => alert.status === "open"));
    const inMaintenance = $derived(node?.maintenance_until != null && new Date(node.maintenance_until).getTime() > now);
    const maintenanceRemaining = $derived(inMaintenance && node?.maintenance_until ? maintenanceLabel(node.maintenance_until) || null : null);
    const tabs = $derived(
        NODE_TABS.map((tab) => ({ ...tab, label: tab.id === "alerts" && openAlerts.length ? `Alerts ${openAlerts.length}` : tab.label })),
    );

    async function load<T>(task: (token: string) => Promise<T>, apply: (value: T) => void, fail: (message: string) => void, fallback: string) {
        if (!auth.token) return;
        try {
            apply(await task(auth.token));
        } catch (e) {
            fail(e instanceof Error ? e.message : fallback);
        }
    }

    async function loadNode() {
        await load((token) => getNode(token, nodeId), (value) => (node = value), (message) => (error = message), "Failed to load node");
        loading = false;
    }

    async function loadAlerts() {
        await load((token) => listEvents(token, { agentId: nodeId }), (value) => (alerts = value), (message) => (alertsError = message), "Failed to load alerts");
        alertsLoading = false;
    }

    async function loadWorkloads() {
        await load(
            (token) => listWorkloads(token),
            (value) => (workloads = value.filter((workload) => workload.assigned_agent_id === nodeId)),
            (message) => (workloadsError = message),
            "Failed to load workloads",
        );
        workloadsLoading = false;
    }

    function applyDisks(value: DiskInfo[]) {
        disks = value;
        disksLoaded = true;
    }

    async function loadDisks() {
        if (disksLoaded) return;
        disksLoading = true;
        await load((token) => getNodeDisks(token, nodeId), applyDisks, (message) => (disksError = message), "Failed to load disks");
        disksLoading = false;
    }

    async function resolveAlert(eventId: string) {
        await load((token) => resolveEvent(token, eventId), () => loadAlerts(), (message) => (alertsError = message), "Failed to resolve alert");
    }

    async function runAction(task: (token: string, id: string) => Promise<void>, fallback: string, confirmation?: string, refresh = true) {
        if (!auth.token || !node || (confirmation && !confirm(confirmation))) return;
        actionBusy = true;
        actionError = null;
        try {
            await task(auth.token, node.id);
            if (refresh) node = await getNode(auth.token, nodeId);
        } catch (e) {
            actionError = e instanceof Error ? e.message : fallback;
        } finally {
            actionBusy = false;
        }
    }

    async function startMaintenance(minutes: number) {
        maintenanceError = null;
        await runAction((token, id) => setMaintenance(token, id, minutes), "Failed to set maintenance");
        if (!actionError) maintenanceDialog?.close();
        maintenanceError = actionError;
    }

    async function removeFromCluster() {
        await runAction(
            async (token, id) => {
                await removeNode(token, id);
                await goto("/nodes");
            },
            "Failed to remove node",
            `Remove ${node?.hostname} from the cluster? Its certificate is revoked and it must join again to return.`,
            false,
        );
    }

    const actions: NodeActions = {
        startMaintenance: () => {
            maintenanceError = null;
            maintenanceDialog?.open();
        },
        endMaintenance: () => runAction(clearNodeMaintenance, "Failed to clear maintenance"),
        reboot: () => runAction(rebootNode, "Failed to reboot node", `Reboot ${node?.hostname}? This will restart the host and briefly disconnect all workloads on it.`),
        cordon: () => runAction(cordonNode, "Failed to cordon node"),
        uncordon: () => runAction(uncordonNode, "Failed to uncordon node"),
        drain: () => runAction(drainNode, "Failed to drain node", `Drain ${node?.hostname}? All workloads will be rescheduled to other nodes and this node will stop receiving new ones.`),
        remove: removeFromCluster,
        powerOff: () => runAction(powerOffNode, "Failed to power off node", `Power off ${node?.hostname}? The host will shut down and stop all workloads on it.`),
    };

    $effect(() => {
        const token = auth.token;
        if (!token) return;
        untrack(() => {
            loadNode();
            loadAlerts();
            loadWorkloads();
            live.start(token);
        });
        const timer = setInterval(() => (now = Date.now()), MAINTENANCE_TICK_MS);
        return () => {
            live.stop();
            clearInterval(timer);
        };
    });

    $effect(() => {
        if (activeTab === "storage" && auth.token) loadDisks();
    });

    $effect(() => {
        breadcrumb.set(node?.hostname ?? null);
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    {#if loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if error || !node}
        <p class="cx-hint cx-hint--error">{error ?? "Node not found"}</p>
    {:else}
        <NodeDetailHeader
            {node}
            uptimeSeconds={live.metrics?.uptime_seconds ?? null}
            {maintenanceRemaining}
            {inMaintenance}
            busy={actionBusy}
            {actions}
        />

        {#if actionError}<p class="cx-hint cx-hint--error">{actionError}</p>{/if}

        <div class="nd-tabs">
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Node sections">
                {#each tabs as tab (tab.id)}
                    <button class="cx-tab" role="tab" type="button" aria-selected={activeTab === tab.id} onclick={() => (activeTab = tab.id)}>
                        {tab.label}
                    </button>
                {/each}
            </div>
        </div>

        {#if activeTab === "summary"}
            <NodeSummaryTab {node} metrics={live.metrics} history={live.history} {openAlerts} {alertsLoading} onviewalerts={() => (activeTab = "alerts")} />
        {:else if activeTab === "hardware"}
            {#if live.metrics}
                <NodeHardwareTab {node} metrics={live.metrics} />
            {:else}
                <p class="cx-hint">{live.error ?? "Loading"}</p>
            {/if}
        {:else if activeTab === "storage"}
            <NodeStorageTab hostname={node.hostname} {disks} loading={disksLoading} error={disksError} />
        {:else if activeTab === "workloads"}
            <NodeWorkloadsTab hostname={node.hostname} {workloads} loading={workloadsLoading} error={workloadsError} />
        {:else if activeTab === "network"}
            {#if live.metrics}
                <NodeNetworkTab {node} metrics={live.metrics} history={live.history} />
            {:else}
                <p class="cx-hint">{live.error ?? "Loading"}</p>
            {/if}
        {:else if activeTab === "alerts"}
            <NodeAlertsTab {alerts} loading={alertsLoading} error={alertsError} onresolve={resolveAlert} />
        {:else}
            <NodeTasksTab />
        {/if}
    {/if}
</div>

<NodeMaintenanceDialog bind:this={maintenanceDialog} busy={actionBusy} error={maintenanceError} onsubmit={startMaintenance} />
