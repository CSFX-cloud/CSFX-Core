<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import Icon from "@iconify/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteStack, getStack, restartStack, stopStack, type Stack } from "$lib/api/resource-groups";
    import { resolveImageIcon } from "$lib/utils/image-icon";
    import { stackStatus } from "$lib/utils/resource-items";
    import { Button } from "$lib/components/ui/button/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import ComposeEditor from "$lib/components/resource-group/compose-editor.svelte";
    import StackLogsTab from "$lib/components/resource-group/stack-logs-tab.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    type Tab = "logs" | "services" | "compose";

    const POLL_INTERVAL_MS = 5000;
    const TABS: [Tab, string][] = [
        ["logs", "Logs"],
        ["services", "Services"],
        ["compose", "Compose"],
    ];

    const rg = useResourceGroupState();
    const stackId = $page.params.stackId ?? "";
    const base = `/resource-groups/${rg.rgId}`;

    let stack = $state<Stack | null>(null);
    let tab = $state<Tab>("logs");
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);

    const children = $derived(rg.workloads.filter((w) => w.stack_id === stackId));

    async function runAction(action: (token: string, id: string) => Promise<unknown>, failure: string) {
        if (!auth.token) return false;
        actionBusy = true;
        actionError = null;
        try {
            await action(auth.token, stackId);
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
        if (await runAction(deleteStack, "Failed to delete stack")) goto(base);
    }

    async function done() {
        await rg.load();
        if (auth.token) stack = await getStack(auth.token, stackId);
        tab = "logs";
    }

    onMount(async () => {
        if (auth.token) stack = await getStack(auth.token, stackId);
    });

    const pollInterval = setInterval(rg.load, POLL_INTERVAL_MS);

    onDestroy(() => clearInterval(pollInterval));
</script>

<header class="flex shrink-0 items-center gap-3 px-4 py-4">
    <Button variant="ghost" size="icon-sm" onclick={() => goto(base)} aria-label="Back to resource group">
        <Icon icon="mdi:arrow-left" width={16} height={16} />
    </Button>
    <Icon icon="logos:docker-icon" width={28} height={28} class="shrink-0" />
    <div class="min-w-0">
        <div class="flex items-center gap-2">
            <span class="text-base font-semibold leading-tight truncate">{stack?.name ?? "Stack"}</span>
            {#if children.length > 0}
                <StatusBadge
                    status={stackStatus(children)}
                    label="{children.filter((c) => c.status === 'running').length}/{children.length} running"
                />
            {/if}
        </div>
        <p class="text-xs text-muted-foreground">{children.length} services</p>
    </div>
    <div class="ml-auto flex items-center gap-0.5 shrink-0">
        <Button variant="ghost" size="icon-sm" onclick={() => runAction(restartStack, "Failed to restart stack")} disabled={actionBusy} aria-label="Restart stack" title="Restart stack">
            <Icon icon="mdi:restart" width={16} height={16} />
        </Button>
        <Button variant="ghost" size="icon-sm" onclick={() => runAction(stopStack, "Failed to stop stack")} disabled={actionBusy} aria-label="Stop stack" title="Stop stack">
            <Icon icon="mdi:stop-circle-outline" width={16} height={16} />
        </Button>
        <Button variant="ghost" size="icon-sm" onclick={remove} disabled={actionBusy} class="text-red-500 hover:text-red-500" aria-label="Delete stack" title="Delete stack">
            <Icon icon="mdi:trash-can-outline" width={16} height={16} />
        </Button>
    </div>
</header>

{#if actionError}
    <p class="text-xs text-destructive px-4 pt-2">{actionError}</p>
{/if}

<div class="flex-1 overflow-y-auto">
    <div class="px-6 pt-4">
        <div class="flex gap-0 border-b">
            {#each TABS as [key, label] (key)}
                <button
                    class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {tab === key ? 'border-foreground text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}"
                    onclick={() => (tab = key)}
                >
                    {label}
                </button>
            {/each}
        </div>
    </div>

    {#if rg.loading}
        <p class="px-6 py-8 text-sm text-muted-foreground">Loading...</p>
    {:else if tab === "logs"}
        <div class="px-6 py-4">
            <div class="border rounded-lg overflow-hidden h-[36rem]">
                {#key children.length}
                    <StackLogsTab workloads={children} />
                {/key}
            </div>
        </div>
    {:else if tab === "services"}
        <div class="px-6 py-4">
            <div class="border rounded-lg divide-y">
                {#each children as child (child.id)}
                    <button
                        class="flex w-full items-center gap-3 p-3 text-left hover:bg-muted/20 transition-colors"
                        onclick={() => goto(`${base}/container/${child.id}`)}
                    >
                        <Icon icon={resolveImageIcon(child.image)} width={20} height={20} class="shrink-0" />
                        <div class="min-w-0 flex-1">
                            <p class="text-sm font-medium leading-tight">{child.service_name ?? child.name}</p>
                            <p class="text-xs text-muted-foreground font-mono truncate">{child.image}</p>
                        </div>
                        <StatusBadge status={child.status} />
                    </button>
                {/each}
            </div>
        </div>
    {:else}
        <ComposeEditor rgId={rg.rgId} {stackId} onDone={done} />
    {/if}
</div>
