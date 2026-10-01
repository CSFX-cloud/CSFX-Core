<script lang="ts">
    import { onDestroy } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import {
        listResourceGroupWorkloads,
        stopWorkload,
        restartWorkload,
        deleteWorkload,
        type Workload,
    } from "$lib/api/resource-groups";
    import { getNode } from "$lib/api/nodes";
    import { fmtBytes } from "$lib/utils/format";
    import { Button } from "$lib/components/ui/button/index.js";
    import VncConsole from "$lib/components/vnc-console.svelte";
    import VmConsolePreview from "$lib/components/vm/vm-console-preview.svelte";
    import VmDetailsTab from "$lib/components/vm/vm-details-tab.svelte";
    import VmSummaryTab from "$lib/components/vm/vm-summary-tab.svelte";
    import ArrowLeftIcon from "@lucide/svelte/icons/arrow-left";
    import CpuIcon from "@lucide/svelte/icons/cpu";
    import GlobeIcon from "@lucide/svelte/icons/globe";
    import MemoryStickIcon from "@lucide/svelte/icons/memory-stick";
    import RotateCwIcon from "@lucide/svelte/icons/rotate-cw";
    import SquareIcon from "@lucide/svelte/icons/square";
    import Trash2Icon from "@lucide/svelte/icons/trash-2";

    type Tab = "summary" | "console" | "details";

    const POLL_INTERVAL_MS = 5000;
    const TABS: { id: Tab; label: string }[] = [
        { id: "summary", label: "Summary" },
        { id: "console", label: "Console" },
        { id: "details", label: "Details" },
    ];
    const STATUS_DOT: Record<string, string> = {
        running: "bg-green-500",
        failed: "bg-red-500",
        error: "bg-red-500",
        pending: "bg-yellow-500",
        starting: "bg-yellow-500",
    };

    const rgId: string = $page.params.id;
    const workloadId: string = $page.params.workloadId;

    let workload = $state<Workload | null>(null);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);
    let nodeIp = $state<string | null>(null);
    let activeTab = $state<Tab>("summary");

    const statusDot = $derived(STATUS_DOT[workload?.status.toLowerCase() ?? ""] ?? "bg-muted-foreground");

    async function loadNodeIp(agentId: string | null) {
        if (!agentId || !auth.token) return;
        try {
            nodeIp = (await getNode(auth.token, agentId)).ip_address;
        } catch {
            nodeIp = null;
        }
    }

    async function load() {
        if (!auth.token) return;
        try {
            const workloads = await listResourceGroupWorkloads(auth.token, rgId);
            workload = workloads.find((w) => w.id === workloadId) ?? null;
            await loadNodeIp(workload?.assigned_agent_id ?? null);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load vm";
        } finally {
            loading = false;
        }
    }

    async function runAction(action: (token: string, id: string) => Promise<unknown>, failure: string) {
        if (!auth.token || !workload) return;
        actionBusy = true;
        actionError = null;
        try {
            await action(auth.token, workload.id);
            await load();
        } catch (e) {
            actionError = e instanceof Error ? e.message : failure;
        } finally {
            actionBusy = false;
        }
    }

    async function handleDelete() {
        if (!auth.token || !workload) return;
        actionBusy = true;
        actionError = null;
        try {
            await deleteWorkload(auth.token, workload.id);
            goto(`/resource-groups/${rgId}`);
        } catch (e) {
            actionError = e instanceof Error ? e.message : "Failed to delete vm";
            actionBusy = false;
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
        if (workload && !pollInterval) pollInterval = setInterval(load, POLL_INTERVAL_MS);
    });

    onDestroy(() => {
        if (pollInterval) clearInterval(pollInterval);
    });
</script>

<header class="flex shrink-0 items-start gap-3 px-4 py-4">
    <Button variant="ghost" size="icon-sm" onclick={() => goto(`/resource-groups/${rgId}`)} aria-label="Back to resource group">
        <ArrowLeftIcon class="size-4" />
    </Button>
    {#if workload}
        <VmConsolePreview {workload} onOpen={() => (activeTab = "console")} />
        <div class="flex flex-col gap-0.5 min-w-0">
            <div class="flex items-center gap-2 min-w-0">
                <span class="text-base font-semibold leading-tight truncate">{workload.service_name ?? workload.name}</span>
                <span class="inline-block w-2 h-2 rounded-full shrink-0 {statusDot}" title={workload.status}></span>
                {#if workload.restart_count > 0}
                    <span class="text-xs px-1.5 py-0.5 rounded-full font-medium bg-amber-500/10 text-amber-600 shrink-0">
                        ↻ {workload.restart_count}
                    </span>
                {/if}
            </div>
            <div class="flex items-center gap-3 text-xs text-muted-foreground leading-tight">
                <span class="flex items-center gap-1">
                    <GlobeIcon class="size-3" />
                    {nodeIp ?? "no node"}
                </span>
                <span class="flex items-center gap-1">
                    <CpuIcon class="size-3" />
                    {workload.cpu_millicores / 1000} vCPU
                </span>
                <span class="flex items-center gap-1">
                    <MemoryStickIcon class="size-3" />
                    {fmtBytes(workload.memory_bytes)}
                </span>
            </div>
        </div>
        <div class="ml-auto flex items-center gap-0.5 shrink-0">
            <Button variant="ghost" size="icon-sm" onclick={() => runAction(restartWorkload, "Failed to restart vm")} disabled={actionBusy} aria-label="Restart" title="Restart">
                <RotateCwIcon class="size-4" />
            </Button>
            <Button variant="ghost" size="icon-sm" onclick={() => runAction(stopWorkload, "Failed to stop vm")} disabled={actionBusy || workload.desired_state === "stopped"} aria-label="Stop" title="Stop">
                <SquareIcon class="size-4" />
            </Button>
            <Button variant="ghost" size="icon-sm" onclick={handleDelete} disabled={actionBusy} class="text-red-500 hover:text-red-500" aria-label="Delete" title="Delete">
                <Trash2Icon class="size-4" />
            </Button>
        </div>
    {/if}
</header>

{#if actionError}
    <p class="text-xs text-destructive px-4 pt-2">{actionError}</p>
{/if}

<div class="flex-1 overflow-y-auto">
    {#if loading}
        <p class="px-6 py-8 text-sm text-muted-foreground">Loading...</p>
    {:else if error}
        <p class="px-6 py-8 text-sm text-destructive">{error}</p>
    {:else if !workload}
        <p class="px-6 py-8 text-sm text-muted-foreground">VM not found.</p>
    {:else}
        <div class="px-6 pt-4 pb-0">
            <div class="flex gap-0 border-b">
                {#each TABS as tab (tab.id)}
                    <button
                        class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {activeTab === tab.id ? 'border-foreground text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}"
                        onclick={() => (activeTab = tab.id)}
                    >
                        {tab.label}
                    </button>
                {/each}
            </div>
        </div>

        {#if activeTab === "summary"}
            <VmSummaryTab {workload} />
        {:else if activeTab === "console"}
            <div class="px-6 py-4">
                <div class="border rounded-lg overflow-hidden h-[36rem]">
                    {#if workload.status !== "running" || !auth.token}
                        <div class="flex items-center justify-center h-full">
                            <p class="text-sm text-muted-foreground">Console is only available while the vm is running.</p>
                        </div>
                    {:else}
                        {#key workload.id}
                            <VncConsole token={auth.token} workloadId={workload.id} />
                        {/key}
                    {/if}
                </div>
            </div>
        {:else}
            <VmDetailsTab {workload} {nodeIp} />
        {/if}
    {/if}
</div>
