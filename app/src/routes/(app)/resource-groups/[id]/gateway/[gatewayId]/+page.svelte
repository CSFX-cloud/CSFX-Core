<script lang="ts">
    import { untrack } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import Icon from "@iconify/svelte";
    import Trash2Icon from "@lucide/svelte/icons/trash-2";
    import { auth } from "$lib/auth/store.svelte";
    import { createIngress, deleteGateway, deleteIngress, listIngress, type IngressRoute } from "$lib/api/ingress";
    import { Button } from "$lib/components/ui/button/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import ContainerLogsTab from "$lib/components/resource-group/container-logs-tab.svelte";
    import DnsRecordsPanel from "$lib/components/resource-group/dns-records-panel.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    type Tab = "routes" | "logs" | "dns";

    const rg = useResourceGroupState();
    const gatewayId = $page.params.gatewayId ?? "";
    const base = `/resource-groups/${rg.rgId}`;
    const tabs: [Tab, string][] = [
        ["routes", "Routes"],
        ["logs", "Logs"],
        ["dns", "Local DNS"],
    ];

    let tab = $state<Tab>("routes");
    let routes = $state<IngressRoute[]>([]);
    let busy = $state(false);
    let error = $state<string | null>(null);
    let loadStarted = false;

    let hostname = $state("");
    let workloadId = $state("");
    let containerPort = $state<number | null>(null);
    let tls = $state(true);

    const gateway = $derived(rg.gateways.find((candidate) => candidate.id === gatewayId) ?? null);
    const gatewayWorkload = $derived(rg.workloads.find((workload) => workload.id === gateway?.workload_id) ?? null);
    const candidates = $derived(
        rg.workloads.filter((workload) => workload.id !== gateway?.workload_id && (workload.ports ?? []).length > 0),
    );
    const ports = $derived(candidates.find((workload) => workload.id === workloadId)?.ports ?? []);
    const canSubmit = $derived(hostname.trim() !== "" && workloadId !== "" && containerPort !== null && !busy);

    $effect(() => {
        void workloadId;
        containerPort = untrack(() => ports[0]?.container_port ?? null);
    });

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            untrack(refresh);
        }
    });

    function workloadName(id: string): string {
        return rg.workloads.find((workload) => workload.id === id)?.name ?? id;
    }

    async function refresh() {
        if (!auth.token) return;
        try {
            const all = await listIngress(auth.token, rg.rgId);
            routes = all.filter((route) => route.gateway_id === gatewayId);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load routes";
        }
    }

    async function handleCreate() {
        if (!auth.token || containerPort === null) return;
        busy = true;
        try {
            await createIngress(auth.token, rg.rgId, {
                hostname: hostname.trim(),
                gateway_id: gatewayId,
                workload_id: workloadId,
                container_port: containerPort,
                tls,
            });
            hostname = "";
            await refresh();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create route";
        } finally {
            busy = false;
        }
    }

    async function handleDeleteRoute(route: IngressRoute) {
        if (!auth.token) return;
        if (!confirm(`Remove ${route.hostname}?`)) return;
        try {
            await deleteIngress(auth.token, rg.rgId, route.id);
            await refresh();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete route";
        }
    }

    async function handleDeleteGateway() {
        if (!auth.token || !gateway) return;
        if (!confirm(`Remove gateway ${gateway.name} and all routes behind it?`)) return;
        try {
            await deleteGateway(auth.token, rg.rgId, gateway.id);
            await rg.load();
            goto(base);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete gateway";
        }
    }
</script>

<header class="flex shrink-0 items-center gap-3 px-4 py-4">
    <Button variant="ghost" size="icon-sm" onclick={() => goto(base)} aria-label="Back to resource group">
        <Icon icon="mdi:arrow-left" width={16} height={16} />
    </Button>
    {#if gateway}
        <Icon icon="mdi:router-network" width={28} height={28} class="shrink-0" />
        <div class="min-w-0">
            <div class="flex items-center gap-2">
                <span class="text-base font-semibold leading-tight truncate">{gateway.name}</span>
                {#if gatewayWorkload}
                    <StatusBadge status={gatewayWorkload.status} class="shrink-0" />
                {/if}
            </div>
            <p class="text-xs text-muted-foreground font-mono truncate">{gateway.endpoint} - ports {gateway.http_port} / {gateway.https_port}</p>
        </div>
        <div class="ml-auto flex items-center gap-0.5 shrink-0">
            <Button variant="ghost" size="icon-sm" onclick={handleDeleteGateway} class="text-red-500 hover:text-red-500" aria-label="Delete gateway" title="Delete gateway">
                <Icon icon="mdi:trash-can-outline" width={16} height={16} />
            </Button>
        </div>
    {/if}
</header>

<div class="flex-1 overflow-y-auto">
    {#if rg.loading}
        <p class="px-6 py-8 text-sm text-muted-foreground">Loading...</p>
    {:else if !gateway}
        <p class="px-6 py-8 text-sm text-muted-foreground">Gateway not found.</p>
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

        <div class="flex flex-col gap-4 px-6 py-4">
            {#if error}
                <p class="text-xs text-destructive">{error}</p>
            {/if}

            {#if tab === "routes"}
                <p class="text-sm text-muted-foreground">
                    Connect services of this resource group to the gateway. Hostnames inside the local zone resolve automatically, point other hostnames at the agent that runs the gateway. Changes reach the gateway within about ten seconds.
                </p>

                <div class="grid gap-3 rounded-lg border p-4 sm:grid-cols-5 sm:items-end">
                    <div class="flex flex-col gap-1 sm:col-span-2">
                        <label class="text-xs text-muted-foreground" for="route-hostname">Hostname</label>
                        <input id="route-hostname" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="app.csfx.internal" bind:value={hostname} />
                    </div>
                    <div class="flex flex-col gap-1">
                        <label class="text-xs text-muted-foreground" for="route-workload">Service</label>
                        <select id="route-workload" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={workloadId}>
                            <option value="">Select</option>
                            {#each candidates as workload}
                                <option value={workload.id}>{workload.name}</option>
                            {/each}
                        </select>
                    </div>
                    <div class="flex flex-col gap-1">
                        <label class="text-xs text-muted-foreground" for="route-port">Port</label>
                        <select id="route-port" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={containerPort} disabled={ports.length === 0}>
                            {#each ports as mapping}
                                <option value={mapping.container_port}>{mapping.container_port}</option>
                            {/each}
                        </select>
                    </div>
                    <div class="flex items-center gap-3">
                        <label class="flex items-center gap-2 text-sm">
                            <input type="checkbox" bind:checked={tls} />
                            TLS
                        </label>
                        <Button size="sm" onclick={handleCreate} disabled={!canSubmit}>Connect</Button>
                    </div>
                </div>

                {#if routes.length === 0}
                    <p class="text-sm text-muted-foreground">No services connected yet.</p>
                {:else}
                    <div class="border rounded-lg overflow-hidden">
                        <table class="w-full text-sm">
                            <thead class="bg-muted/30 text-xs text-muted-foreground">
                                <tr>
                                    <th class="px-4 py-2 text-left font-medium">Hostname</th>
                                    <th class="px-4 py-2 text-left font-medium">Service</th>
                                    <th class="px-4 py-2 text-left font-medium">Port</th>
                                    <th class="px-4 py-2 text-left font-medium">TLS</th>
                                    <th class="px-4 py-2"></th>
                                </tr>
                            </thead>
                            <tbody>
                                {#each routes as route}
                                    <tr class="border-t">
                                        <td class="px-4 py-2 font-mono text-xs">{route.hostname}</td>
                                        <td class="px-4 py-2">{workloadName(route.workload_id)}</td>
                                        <td class="px-4 py-2">{route.container_port}</td>
                                        <td class="px-4 py-2">{route.tls ? "yes" : "no"}</td>
                                        <td class="px-4 py-2 text-right">
                                            <Button variant="ghost" size="icon-sm" onclick={() => handleDeleteRoute(route)} aria-label="Remove route" title="Remove route">
                                                <Trash2Icon class="size-4" />
                                            </Button>
                                        </td>
                                    </tr>
                                {/each}
                            </tbody>
                        </table>
                    </div>
                {/if}
            {:else if tab === "logs"}
                <ContainerLogsTab workloadId={gateway.workload_id} running />
            {:else}
                <DnsRecordsPanel groupId={rg.rgId} />
            {/if}
        </div>
    {/if}
</div>
