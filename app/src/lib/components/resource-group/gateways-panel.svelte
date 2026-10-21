<script lang="ts">
    import Trash2Icon from "@lucide/svelte/icons/trash-2";
    import { auth } from "$lib/auth/store.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import { createGateway, deleteGateway, type Gateway } from "$lib/api/ingress";

    let { groupId, gateways, onchange }: { groupId: string; gateways: Gateway[]; onchange: () => Promise<void> } = $props();

    let name = $state("");
    let httpPort = $state(80);
    let httpsPort = $state(443);
    let busy = $state(false);
    let error = $state<string | null>(null);

    const canSubmit = $derived(name.trim() !== "" && httpPort !== httpsPort && !busy);

    async function handleCreate() {
        if (!auth.token) return;
        busy = true;
        try {
            await createGateway(auth.token, groupId, { name: name.trim(), http_port: httpPort, https_port: httpsPort });
            name = "";
            error = null;
            await onchange();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create gateway";
        } finally {
            busy = false;
        }
    }

    async function handleDelete(gateway: Gateway) {
        if (!auth.token) return;
        if (!confirm(`Remove gateway ${gateway.name} and all routes behind it?`)) return;
        try {
            await deleteGateway(auth.token, groupId, gateway.id);
            error = null;
            await onchange();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete gateway";
        }
    }
</script>

<section class="flex flex-col gap-3">
    <div>
        <h2 class="text-base font-semibold tracking-tight">Gateways</h2>
        <p class="text-sm text-muted-foreground mt-0.5">
            A gateway is a reverse proxy that runs as a service in this resource group. Hostnames are reachable once a gateway serves them.
        </p>
    </div>

    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}

    <div class="grid gap-3 rounded-lg border p-4 sm:grid-cols-4 sm:items-end">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="gateway-name">Name</label>
            <input id="gateway-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="edge" bind:value={name} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="gateway-http">HTTP port</label>
            <input id="gateway-http" type="number" min="1" max="65535" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={httpPort} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="gateway-https">HTTPS port</label>
            <input id="gateway-https" type="number" min="1" max="65535" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={httpsPort} />
        </div>
        <div>
            <Button size="sm" onclick={handleCreate} disabled={!canSubmit}>Create gateway</Button>
        </div>
    </div>

    {#if gateways.length === 0}
        <p class="text-sm text-muted-foreground">No gateways yet. Create one before adding routes.</p>
    {:else}
        <div class="border rounded-lg overflow-hidden">
            <table class="w-full text-sm">
                <thead class="bg-muted/30 text-xs text-muted-foreground">
                    <tr>
                        <th class="px-4 py-2 text-left font-medium">Name</th>
                        <th class="px-4 py-2 text-left font-medium">Status</th>
                        <th class="px-4 py-2 text-left font-medium">Ports</th>
                        <th class="px-4 py-2 text-left font-medium">Endpoint</th>
                        <th class="px-4 py-2"></th>
                    </tr>
                </thead>
                <tbody>
                    {#each gateways as gateway}
                        <tr class="border-t">
                            <td class="px-4 py-2">{gateway.name}</td>
                            <td class="px-4 py-2">{gateway.status ?? "unknown"}</td>
                            <td class="px-4 py-2">{gateway.http_port} / {gateway.https_port}</td>
                            <td class="px-4 py-2 font-mono text-xs">{gateway.endpoint}</td>
                            <td class="px-4 py-2 text-right">
                                <Button variant="ghost" size="icon-sm" onclick={() => handleDelete(gateway)} aria-label="Remove gateway" title="Remove gateway">
                                    <Trash2Icon class="size-4" />
                                </Button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</section>
