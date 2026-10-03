<script lang="ts">
    import { onDestroy } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import Icon from "@iconify/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteWorkload, restartWorkload, stopWorkload } from "$lib/api/resource-groups";
    import { resolveImageIcon } from "$lib/utils/image-icon";
    import { Button } from "$lib/components/ui/button/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import ContainerInsightsTab from "$lib/components/resource-group/container-insights-tab.svelte";
    import ContainerLogsTab from "$lib/components/resource-group/container-logs-tab.svelte";
    import ContainerNetworkTab from "$lib/components/resource-group/container-network-tab.svelte";
    import ContainerSettingsTab from "$lib/components/resource-group/container-settings-tab.svelte";
    import ContainerShellTab from "$lib/components/resource-group/container-shell-tab.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    type Tab = "logs" | "shell" | "insights" | "network" | "settings";

    const POLL_INTERVAL_MS = 5000;
    const rg = useResourceGroupState();
    const workloadId = $page.params.workloadId;

    let tab = $state<Tab>("logs");
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);

    const container = $derived(rg.workloads.find((w) => w.id === workloadId) ?? null);

    const tabs = $derived<[Tab, string][]>([
        ["logs", "Logs"],
        ["shell", "Shell"],
        ["insights", "Performance"],
        ["network", "Network"],
        ...(container?.stack_id ? [] : [["settings", "Settings"] as [Tab, string]]),
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
        if (await runAction(deleteWorkload, "Failed to delete container")) goto(`/resource-groups/${rg.rgId}`);
    }

    const pollInterval = setInterval(rg.load, POLL_INTERVAL_MS);

    onDestroy(() => clearInterval(pollInterval));
</script>

<header class="flex shrink-0 items-center gap-3 px-4 py-4">
    <Button variant="ghost" size="icon-sm" onclick={() => goto(`/resource-groups/${rg.rgId}`)} aria-label="Back to resource group">
        <Icon icon="mdi:arrow-left" width={16} height={16} />
    </Button>
    {#if container}
        <Icon icon={resolveImageIcon(container.image)} width={28} height={28} class="shrink-0" />
        <div class="min-w-0">
            <div class="flex items-center gap-2">
                <span class="text-base font-semibold leading-tight truncate">{container.service_name ?? container.name}</span>
                <StatusBadge status={container.status} class="shrink-0" />
            </div>
            <p class="text-xs text-muted-foreground font-mono truncate">{container.image}</p>
        </div>
        <div class="ml-auto flex items-center gap-0.5 shrink-0">
            <Button variant="ghost" size="icon-sm" onclick={() => runAction(restartWorkload, "Failed to restart container")} disabled={actionBusy} aria-label="Restart" title="Restart">
                <Icon icon="mdi:restart" width={16} height={16} />
            </Button>
            <Button variant="ghost" size="icon-sm" onclick={() => runAction(stopWorkload, "Failed to stop container")} disabled={actionBusy || container.desired_state === "stopped"} aria-label="Stop" title="Stop">
                <Icon icon="mdi:stop-circle-outline" width={16} height={16} />
            </Button>
            <Button variant="ghost" size="icon-sm" onclick={remove} disabled={actionBusy} class="text-red-500 hover:text-red-500" aria-label="Delete" title="Delete">
                <Icon icon="mdi:trash-can-outline" width={16} height={16} />
            </Button>
        </div>
    {/if}
</header>

{#if actionError}
    <p class="text-xs text-destructive px-4 pt-2">{actionError}</p>
{/if}

<div class="flex-1 overflow-y-auto">
    {#if rg.loading}
        <p class="px-6 py-8 text-sm text-muted-foreground">Loading...</p>
    {:else if !container}
        <p class="px-6 py-8 text-sm text-muted-foreground">Container not found.</p>
    {:else}
        <div class="px-6 pt-4">
            <div class="flex gap-0 border-b">
                {#each tabs as [key, label] (key)}
                    <button
                        class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {tab === key ? 'border-foreground text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}"
                        onclick={() => (tab = key)}
                    >
                        {label}
                    </button>
                {/each}
            </div>
        </div>
        <div class="px-6 py-4">
            <div class="border rounded-lg overflow-hidden h-[36rem]">
                {#if tab === "logs"}
                    <ContainerLogsTab workloadId={container.id} />
                {:else if tab === "shell"}
                    <ContainerShellTab workload={container} />
                {:else if tab === "insights"}
                    <ContainerInsightsTab workload={container} />
                {:else if tab === "network"}
                    <ContainerNetworkTab workload={container} rgId={rg.rgId} />
                {:else}
                    <ContainerSettingsTab workload={container} onRedeployed={rg.load} />
                {/if}
            </div>
        </div>
    {/if}
</div>
