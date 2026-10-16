<script lang="ts">
    import { untrack } from "svelte";
    import Trash2Icon from "@lucide/svelte/icons/trash-2";
    import { auth } from "$lib/auth/store.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import { createIngress, deleteIngress, listIngress, type IngressRoute } from "$lib/api/ingress";
    import RgTopbar from "$lib/components/resource-group/rg-topbar.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let routes = $state<IngressRoute[]>([]);
    let loading = $state(true);
    let busy = $state(false);
    let error = $state<string | null>(null);
    let loadStarted = false;

    let hostname = $state("");
    let workloadId = $state("");
    let containerPort = $state<number | null>(null);
    let tls = $state(true);

    const candidates = $derived(rg.workloads.filter((workload) => (workload.ports ?? []).length > 0));
    const ports = $derived(candidates.find((workload) => workload.id === workloadId)?.ports ?? []);
    const canSubmit = $derived(hostname.trim() !== "" && workloadId !== "" && containerPort !== null && !busy);

    $effect(() => {
        void workloadId;
        containerPort = untrack(() => ports[0]?.container_port ?? null);
    });

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            refresh();
        }
    });

    function workloadName(id: string): string {
        return rg.workloads.find((workload) => workload.id === id)?.name ?? id;
    }

    async function refresh() {
        if (!auth.token) return;
        try {
            routes = await listIngress(auth.token, rg.rgId);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load ingress routes";
        } finally {
            loading = false;
        }
    }

    async function handleCreate() {
        if (!auth.token || containerPort === null) return;
        busy = true;
        try {
            await createIngress(auth.token, rg.rgId, {
                hostname: hostname.trim(),
                workload_id: workloadId,
                container_port: containerPort,
                tls,
            });
            hostname = "";
            await refresh();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create ingress route";
        } finally {
            busy = false;
        }
    }

    async function handleDelete(route: IngressRoute) {
        if (!auth.token) return;
        if (!confirm(`Remove ${route.hostname}?`)) return;
        try {
            await deleteIngress(auth.token, rg.rgId, route.id);
            await refresh();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete ingress route";
        }
    }
</script>

<RgTopbar backHref={base} backLabel="Back to resource group" />

<div class="flex min-w-0 flex-1 flex-col gap-6 p-6">
    <div>
        <h1 class="text-xl font-semibold tracking-tight">Ingress</h1>
        <p class="text-sm text-muted-foreground mt-0.5">
            Publish a workload under a hostname. Point the DNS record at the cluster address. Changes reach the proxy within about ten seconds.
        </p>
    </div>

    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}

    <div class="grid gap-3 rounded-lg border p-4 sm:grid-cols-5 sm:items-end">
        <div class="flex flex-col gap-1 sm:col-span-2">
            <label class="text-xs text-muted-foreground" for="ingress-hostname">Hostname</label>
            <input id="ingress-hostname" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="app.example.com" bind:value={hostname} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="ingress-workload">Workload</label>
            <select id="ingress-workload" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={workloadId}>
                <option value="">Select</option>
                {#each candidates as workload}
                    <option value={workload.id}>{workload.name}</option>
                {/each}
            </select>
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="ingress-port">Port</label>
            <select id="ingress-port" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={containerPort} disabled={ports.length === 0}>
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
            <Button size="sm" onclick={handleCreate} disabled={!canSubmit}>Add</Button>
        </div>
    </div>

    {#if loading}
        <p class="text-sm text-muted-foreground">Loading...</p>
    {:else if routes.length === 0}
        <p class="text-sm text-muted-foreground">No ingress routes yet.</p>
    {:else}
        <div class="border rounded-lg overflow-hidden">
            <table class="w-full text-sm">
                <thead class="bg-muted/30 text-xs text-muted-foreground">
                    <tr>
                        <th class="px-4 py-2 text-left font-medium">Hostname</th>
                        <th class="px-4 py-2 text-left font-medium">Workload</th>
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
                                <Button variant="ghost" size="icon-sm" onclick={() => handleDelete(route)} aria-label="Remove route" title="Remove route">
                                    <Trash2Icon class="size-4" />
                                </Button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</div>
