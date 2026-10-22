<script lang="ts">
    import { onDestroy } from "svelte";
    import { Globe, HardDrive, Layers, Monitor, Play, RefreshCw, Square, Trash2 } from "@lucide/svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteWorkload, restartWorkload, stopWorkload } from "$lib/api/resource-groups";
    import { getNode } from "$lib/api/nodes";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import VncConsole from "$lib/components/vnc-console.svelte";
    import VmConsolePreview from "$lib/components/vm/vm-console-preview.svelte";
    import VmDetailsTab from "$lib/components/vm/vm-details-tab.svelte";
    import VmSummaryTab from "$lib/components/vm/vm-summary-tab.svelte";
    import WorkloadHero, { type MetaItem } from "$lib/components/workload/workload-hero.svelte";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";
    import { formatCapacity } from "$lib/utils/format";

    type Tab = "summary" | "console" | "details";

    const POLL_INTERVAL_MS = 5000;
    const MILLICORES_PER_CORE = 1000;
    const TABS: { id: Tab; label: string }[] = [
        { id: "summary", label: "Summary" },
        { id: "console", label: "Console" },
        { id: "details", label: "Details" },
    ];

    const rg = useResourceGroupState();
    const workloadId: string = $page.params.workloadId ?? "";

    let tab = $state<Tab>("summary");
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);
    let nodeIp = $state<string | null>(null);

    const workload = $derived(rg.workloads.find((candidate) => candidate.id === workloadId) ?? null);
    const name = $derived(workload?.service_name ?? workload?.name ?? "");
    const running = $derived(workload?.status === "running");
    const stopped = $derived(workload?.desired_state === "stopped" || workload?.status === "stopped");
    const meta = $derived<MetaItem[]>(
        workload
            ? [
                  { icon: Globe, text: nodeIp ?? "no node" },
                  { icon: Monitor, text: `${workload.cpu_millicores / MILLICORES_PER_CORE} vCPU · ${formatCapacity(workload.memory_bytes)}` },
                  { icon: HardDrive, text: formatCapacity(workload.disk_bytes) },
                  { icon: Layers, text: rg.group?.name ?? "Resource group", href: `/resource-groups/${rg.rgId}` },
              ]
            : [],
    );

    async function runAction(action: (token: string, id: string) => Promise<unknown>, failure: string) {
        if (!auth.token || !workload) return false;
        actionBusy = true;
        actionError = null;
        try {
            await action(auth.token, workload.id);
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
        if (await runAction(deleteWorkload, "Failed to delete VM")) goto(`/resource-groups/${rg.rgId}`);
    }

    $effect(() => {
        const agentId = workload?.assigned_agent_id;
        const token = auth.token;
        if (!agentId || !token) return;
        getNode(token, agentId)
            .then((node) => (nodeIp = node.ip_address))
            .catch(() => (nodeIp = null));
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
    {:else if !workload}
        <p class="cx-hint cx-hint--error">VM not found.</p>
    {:else}
        <WorkloadHero label="Virtual machine {name}" title={name} status={workload.status} {meta}>
            {#snippet lead()}
                <VmConsolePreview {workload} onOpen={() => (tab = "console")} />
            {/snippet}
            {#snippet actions()}
                {#if stopped}
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionBusy} onclick={() => runAction(restartWorkload, "Failed to start VM")}>
                        <Play class="cx-i" aria-hidden="true" />Start VM
                    </button>
                {:else}
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionBusy} onclick={() => runAction(stopWorkload, "Failed to stop VM")}>
                        <Square class="cx-i" aria-hidden="true" />Stop
                    </button>
                {/if}
                <button class="cx-icon-btn" type="button" disabled={actionBusy} aria-label="Restart" title="Restart" onclick={() => runAction(restartWorkload, "Failed to restart VM")}>
                    <RefreshCw class="cx-i" aria-hidden="true" />
                </button>
                <button class="cx-icon-btn nd-danger" type="button" disabled={actionBusy} aria-label="Delete" title="Delete" onclick={remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            {/snippet}
        </WorkloadHero>

        {#if actionError}<p class="cx-hint cx-hint--error">{actionError}</p>{/if}

        <WorkloadTabs tabs={TABS} active={tab} label="VM sections" onselect={(id) => (tab = id)} />

        {#if tab === "summary"}
            <VmSummaryTab {workload} {nodeIp} />
        {:else if tab === "console"}
            {#if !running || !auth.token}
                <section class="cx-stage empty" aria-label="Console unavailable" style="min-height: 420px">
                    <div class="cx-stage__canvas"></div>
                    <div class="empty__in">
                        <span class="glow" aria-hidden="true"><Monitor class="cx-i" /></span>
                        <h2>Start the VM to open its console.</h2>
                        <p>The console shows the VM's screen and takes keyboard input while it runs.</p>
                        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" style="margin-top: 8px" disabled={actionBusy} onclick={() => runAction(restartWorkload, "Failed to start VM")}>
                            <Play class="cx-i" aria-hidden="true" />Start VM
                        </button>
                    </div>
                </section>
            {:else}
                <section class="vm-console" aria-label="Console">
                    <div class="term__bar">
                        <span class="term__ok" style="display: inline-flex; align-items: center; gap: 8px; font: 400 12px/16px var(--font-mono)">
                            <i style="width: 6px; height: 6px; border-radius: 50%; background: var(--success); display: inline-block"></i>Connected
                        </span>
                    </div>
                    <div class="vm-console__screen">
                        {#key workload.id}
                            <VncConsole token={auth.token} workloadId={workload.id} />
                        {/key}
                    </div>
                </section>
            {/if}
        {:else}
            <VmDetailsTab {workload} {nodeIp} groupName={rg.group?.name ?? "Resource group"} groupHref={`/resource-groups/${rg.rgId}`} />
        {/if}
    {/if}
</div>
