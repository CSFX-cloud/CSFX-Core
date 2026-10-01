<script lang="ts">
    import Icon from "@iconify/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteWorkload, restartWorkload, stopWorkload, type Workload } from "$lib/api/resource-groups";
    import { resolveImageIcon } from "$lib/utils/image-icon";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import VncConsole from "$lib/components/vnc-console.svelte";
    import ContainerInsightsTab from "./container-insights-tab.svelte";
    import ContainerLogsTab from "./container-logs-tab.svelte";
    import ContainerNetworkTab from "./container-network-tab.svelte";
    import ContainerSettingsTab from "./container-settings-tab.svelte";
    import ContainerShellTab from "./container-shell-tab.svelte";

    type Tab = "logs" | "shell" | "insights" | "network" | "settings";

    let {
        rgId,
        workloads,
        onChanged,
    }: { rgId: string; workloads: Workload[]; onChanged: () => Promise<void> } = $props();

    let dialog = $state<HTMLDialogElement | null>(null);
    let activeId = $state<string | null>(null);
    let tab = $state<Tab>("logs");
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);

    const container = $derived(workloads.find((w) => w.id === activeId) ?? null);
    const isVm = $derived(container?.runtime_class === "vm");

    const tabs = $derived<[Tab, string][]>(
        container
            ? [
                  ...(isVm ? [] : [["logs", "Logs"] as [Tab, string]]),
                  ["shell", isVm ? "Console" : "Shell"],
                  ["insights", "Performance"],
                  ["network", "Network"],
                  ...(container.stack_id ? [] : [["settings", "Settings"] as [Tab, string]]),
              ]
            : [],
    );

    export function open(workload: Workload) {
        activeId = workload.id;
        actionError = null;
        tab = workload.runtime_class === "vm" ? "shell" : "logs";
        dialog?.showModal();
    }

    function close() {
        dialog?.close();
    }

    async function runAction(action: (token: string, id: string) => Promise<unknown>, failure: string) {
        if (!auth.token || !container) return false;
        actionBusy = true;
        actionError = null;
        try {
            await action(auth.token, container.id);
            await onChanged();
            return true;
        } catch (e) {
            actionError = e instanceof Error ? e.message : failure;
            return false;
        } finally {
            actionBusy = false;
        }
    }

    async function remove() {
        if (await runAction(deleteWorkload, "Failed to delete container")) close();
    }

    const actionButton =
        "flex items-center justify-center w-8 h-8 rounded-full transition-colors disabled:opacity-40 disabled:pointer-events-none";
</script>

<dialog
    bind:this={dialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-4xl h-[80vh] rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => (activeId = null)}
>
    {#if container}
        <div class="flex flex-col h-full">
            <div class="flex items-center justify-between px-6 py-4 border-b shrink-0 gap-4">
                <div class="flex items-center gap-2.5 min-w-0">
                    <Icon icon={isVm ? "mdi:monitor" : resolveImageIcon(container.image)} width={20} height={20} class="shrink-0" />
                    <div class="min-w-0">
                        <h2 class="text-base font-semibold leading-tight truncate">{container.service_name ?? container.name}</h2>
                        <p class="text-xs text-muted-foreground font-mono truncate">{container.image}</p>
                    </div>
                    <StatusBadge status={container.status} class="shrink-0" />
                </div>
                <div class="flex items-center gap-1 shrink-0">
                    <button
                        class="{actionButton} text-muted-foreground hover:text-foreground hover:bg-muted"
                        onclick={() => runAction(restartWorkload, "Failed to restart container")}
                        disabled={actionBusy}
                        aria-label="Restart"
                        title="Restart"
                    >
                        <Icon icon="mdi:restart" width={18} height={18} />
                    </button>
                    <button
                        class="{actionButton} text-muted-foreground hover:text-foreground hover:bg-muted"
                        onclick={() => runAction(stopWorkload, "Failed to stop container")}
                        disabled={actionBusy || container.desired_state === "stopped"}
                        aria-label="Stop"
                        title="Stop"
                    >
                        <Icon icon="mdi:stop-circle-outline" width={18} height={18} />
                    </button>
                    <button
                        class="{actionButton} text-destructive hover:bg-destructive/10"
                        onclick={remove}
                        disabled={actionBusy}
                        aria-label="Delete"
                        title="Delete"
                    >
                        <Icon icon="mdi:trash-can-outline" width={18} height={18} />
                    </button>
                    <div class="w-px h-5 bg-border mx-1"></div>
                    <button
                        class="{actionButton} text-muted-foreground hover:text-foreground hover:bg-muted"
                        onclick={close}
                        aria-label="Close"
                        title="Close"
                    >
                        <Icon icon="mdi:close" width={18} height={18} />
                    </button>
                </div>
            </div>
            {#if actionError}
                <p class="px-6 py-2 text-xs text-destructive shrink-0 border-b">{actionError}</p>
            {/if}
            <div class="px-6 py-2 border-b shrink-0">
                <div class="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-muted">
                    {#each tabs as [key, label]}
                        <button
                            class="px-3 py-1 rounded-md text-sm font-medium transition-all duration-200 {tab === key ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                            onclick={() => (tab = key)}
                        >
                            {label}
                        </button>
                    {/each}
                </div>
            </div>
            <div class="flex-1 overflow-hidden">
                {#if tab === "logs"}
                    <ContainerLogsTab workloadId={container.id} />
                {:else if tab === "shell" && isVm}
                    {#if container.status !== "running" || !auth.token}
                        <p class="p-4 text-xs text-muted-foreground">Console is only available while the vm is running.</p>
                    {:else}
                        {#key container.id}
                            <VncConsole token={auth.token} workloadId={container.id} />
                        {/key}
                    {/if}
                {:else if tab === "shell"}
                    <ContainerShellTab workload={container} />
                {:else if tab === "insights"}
                    <ContainerInsightsTab workload={container} />
                {:else if tab === "network"}
                    <ContainerNetworkTab workload={container} {rgId} />
                {:else if !container.stack_id}
                    <ContainerSettingsTab workload={container} onRedeployed={onChanged} />
                {/if}
            </div>
        </div>
    {/if}
</dialog>
