<script lang="ts">
    import { onMount } from "svelte";
    import Icon from "@iconify/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getNode } from "$lib/api/nodes";
    import type { Workload } from "$lib/api/resource-groups";
    import { createClipboard } from "$lib/utils/clipboard.svelte";

    let { workload, rgId }: { workload: Workload; rgId: string } = $props();

    const clipboard = createClipboard();
    let nodeIp = $state<string | null>(null);

    const host = $derived(`${workload.service_name ?? workload.name}.svc.${rgId}.internal`);

    const rows = $derived(
        (workload.ports ?? []).map((port) => ({
            port: port.container_port,
            addresses: [
                { key: `rg-${port.container_port}`, label: "RG-internal (DNS)", value: `${host}:${port.container_port}` },
                ...(port.rg_port
                    ? [{ key: `rgp-${port.container_port}`, label: "RG port (mesh gateway)", value: `rg-gateway:${port.rg_port}` }]
                    : []),
                ...(port.node_port
                    ? [{ key: `ext-${port.container_port}`, label: "External", value: `${nodeIp ?? "node-ip"}:${port.node_port}` }]
                    : []),
            ],
        })),
    );

    onMount(async () => {
        if (!auth.token || !workload.assigned_agent_id) return;
        try {
            nodeIp = (await getNode(auth.token, workload.assigned_agent_id)).ip_address;
        } catch {
            nodeIp = null;
        }
    });
</script>

<div class="h-full overflow-y-auto p-6 space-y-6">
    {#if rows.length > 0}
        <div>
            <p class="text-xs font-medium text-muted-foreground mb-2">Ports</p>
            <div class="border rounded-lg divide-y">
                {#each rows as row (row.port)}
                    <div class="p-3 space-y-2">
                        {#each row.addresses as address (address.key)}
                            <div class="flex items-center justify-between gap-3">
                                <div class="min-w-0">
                                    <p class="text-xs text-muted-foreground">{address.label}</p>
                                    <p class="font-mono text-sm truncate">{address.value}</p>
                                </div>
                                <button
                                    class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors shrink-0"
                                    onclick={() => clipboard.copy(address.value, address.key)}
                                    aria-label="Copy"
                                    title="Copy"
                                >
                                    <Icon icon={clipboard.copiedKey === address.key ? "mdi:check" : "mdi:content-copy"} width={16} height={16} />
                                </button>
                            </div>
                        {/each}
                    </div>
                {/each}
            </div>
        </div>
        <p class="text-xs text-muted-foreground">
            Reachable over VPN via the RG-internal address, or externally via the node IP if a node port is set. Use "Connect VPN" to resolve RG-internal hostnames.
        </p>
    {:else}
        <p class="text-sm text-muted-foreground">No ports configured for this container.</p>
    {/if}
</div>
