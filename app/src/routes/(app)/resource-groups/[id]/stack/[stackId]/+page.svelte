<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { Layers, Package, RefreshCw, Square, Trash2 } from "@lucide/svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteStack, getStack, restartStack, stopStack, type Stack } from "$lib/api/resource-groups";
    import ComposeEditor from "$lib/components/resource-group/compose-editor.svelte";
    import StackLogsTab from "$lib/components/resource-group/stack-logs-tab.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import WorkloadHero, { type MetaItem } from "$lib/components/workload/workload-hero.svelte";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";
    import { knownWorkloadIcon } from "$lib/utils/image-icon";
    import { stackStatus } from "$lib/utils/resource-items";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    type Tab = "logs" | "services" | "compose";

    const POLL_INTERVAL_MS = 5000;
    const TABS: { id: Tab; label: string }[] = [
        { id: "logs", label: "Logs" },
        { id: "services", label: "Services" },
        { id: "compose", label: "Compose" },
    ];

    const rg = useResourceGroupState();
    const stackId = $page.params.stackId ?? "";
    const base = `/resource-groups/${rg.rgId}`;

    let stack = $state<Stack | null>(null);
    let tab = $state<Tab>("logs");
    let actionError = $state<string | null>(null);
    let actionBusy = $state(false);

    const children = $derived(rg.workloads.filter((workload) => workload.stack_id === stackId));
    const running = $derived(children.filter((child) => child.status === "running").length);
    const name = $derived(stack?.name ?? "Stack");
    const meta = $derived<MetaItem[]>([
        { icon: Package, text: `${children.length} services` },
        { icon: Layers, text: rg.group?.name ?? "Resource group", href: base },
    ]);

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
        if (!confirm(`Delete stack ${name}?`)) return;
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

    $effect(() => {
        breadcrumb.set(rg.group ? `${rg.group.name} / ${name}` : null);
        return () => breadcrumb.set(null);
    });

    const pollInterval = setInterval(rg.load, POLL_INTERVAL_MS);

    onDestroy(() => clearInterval(pollInterval));
</script>

<div class="db-content">
    {#if rg.loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else}
        <WorkloadHero
            label="Compose stack {name}"
            title={name}
            status={children.length ? stackStatus(children) : "pending"}
            statusText={children.length ? `${running}/${children.length} running` : undefined}
            {meta}
        >
            {#snippet lead()}
                <ServiceIcon name="docker-compose" size="xl" />
            {/snippet}
            {#snippet actions()}
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionBusy} onclick={() => runAction(stopStack, "Failed to stop stack")}>
                    <Square class="cx-i" aria-hidden="true" />Stop
                </button>
                <button class="cx-icon-btn" type="button" disabled={actionBusy} aria-label="Restart stack" title="Restart stack" onclick={() => runAction(restartStack, "Failed to restart stack")}>
                    <RefreshCw class="cx-i" aria-hidden="true" />
                </button>
                <button class="cx-icon-btn nd-danger" type="button" disabled={actionBusy} aria-label="Delete stack" title="Delete stack" onclick={remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            {/snippet}
        </WorkloadHero>

        {#if actionError}<p class="cx-hint cx-hint--error">{actionError}</p>{/if}

        <WorkloadTabs tabs={TABS} active={tab} label="Stack sections" onselect={(id) => (tab = id)} />

        {#if tab === "logs"}
            {#key children.length}
                <StackLogsTab workloads={children} />
            {/key}
        {:else if tab === "services"}
            <section class="cx-panel" aria-labelledby="stk-services">
                <div class="cx-panel__head">
                    <div><h2 class="cx-panel__title" id="stk-services">Services</h2><p class="cx-panel__sub">Containers that make up {name}</p></div>
                </div>
                <div class="db-scroll">
                    <table class="cx-table">
                        <thead><tr><th>Service</th><th>Status</th><th>Image</th></tr></thead>
                        <tbody>
                            {#each children as child (child.id)}
                                <tr class="nl-row">
                                    <td>
                                        <div class="rs-name">
                                            <ServiceIcon name="docker-containers" brand={knownWorkloadIcon(child.image, child.runtime_class)} />
                                            <a class="rs-link" href="{base}/container/{child.id}">{child.service_name ?? child.name}</a>
                                        </div>
                                    </td>
                                    <td><span class={badgeClass(child.status)}>{statusLabel(child.status)}</span></td>
                                    <td class="cx-num">{child.image}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            </section>
        {:else}
            <ComposeEditor rgId={rg.rgId} groupName={rg.group?.name ?? "the group"} {stackId} onDone={done} />
        {/if}
    {/if}
</div>
