<script lang="ts">
    import { onMount } from "svelte";
    import { Check, Copy } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getNode } from "$lib/api/nodes";
    import type { Workload } from "$lib/api/resource-groups";
    import { createClipboard } from "$lib/utils/clipboard.svelte";

    let { workload, rgId, groupName, cidr }: { workload: Workload; rgId: string; groupName: string; cidr: string } = $props();

    const clipboard = createClipboard();
    let nodeIp = $state<string | null>(null);

    const name = $derived(workload.service_name ?? workload.name);
    const host = $derived(`${name}.svc.${rgId}.internal`);
    const defaultPort = $derived(workload.ports?.[0]?.container_port ?? null);

    const rows = $derived(
        (workload.ports ?? []).map((port) => ({
            port: port.container_port,
            addresses: [
                { key: `rg-${port.container_port}`, label: "RG-internal (DNS)", value: `${host}:${port.container_port}` },
                ...(port.rg_port ? [{ key: `rgp-${port.container_port}`, label: "RG port (mesh gateway)", value: `rg-gateway:${port.rg_port}` }] : []),
                ...(port.node_port ? [{ key: `ext-${port.container_port}`, label: "External", value: `${nodeIp ?? "node-ip"}:${port.node_port}` }] : []),
            ],
        })),
    );

    onMount(async () => {
        if (!auth.token || !workload.assigned_agent_id) return;
        nodeIp = await getNode(auth.token, workload.assigned_agent_id)
            .then((node) => node.ip_address)
            .catch(() => null);
    });
</script>

<div class="rd-two">
    <section class="cx-panel" aria-labelledby="ct-ports">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="ct-ports">Published ports</h2>
                <p class="cx-panel__sub">Reachable from outside the resource group</p>
            </div>
        </div>
        {#if rows.length === 0}
            <p class="cx-hint" style="padding: 20px 0; text-align: center">No ports published.</p>
        {:else}
            <dl class="nd-kv">
                {#each rows as row (row.port)}
                    {#each row.addresses as address (address.key)}
                        <div>
                            <dt>{address.label}</dt>
                            <dd class="mono" style="display: flex; align-items: center; gap: 8px; justify-content: flex-end">
                                <span>{address.value}</span>
                                <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Copy {address.value}" title="Copy" onclick={() => clipboard.copy(address.value, address.key)}>
                                    {#if clipboard.copiedKey === address.key}<Check class="cx-i" aria-hidden="true" />{:else}<Copy class="cx-i" aria-hidden="true" />{/if}
                                </button>
                            </dd>
                        </div>
                    {/each}
                {/each}
            </dl>
        {/if}
    </section>
    <section class="cx-panel" aria-labelledby="ct-internal">
        <div class="cx-panel__head"><h2 class="cx-panel__title" id="ct-internal">Inside {groupName}</h2></div>
        <dl class="nd-kv">
            <div><dt>Internal DNS name</dt><dd class="mono">{name}</dd></div>
            <div><dt>Default port</dt><dd class="mono">{defaultPort ?? "—"}</dd></div>
            <div><dt>Network</dt><dd class="mono">{cidr}</dd></div>
        </dl>
        <p class="cx-hint" style="margin-top: 16px">
            Other resources in {groupName} reach this container by its name on its default port. Use "Connect VPN" to resolve RG-internal hostnames.
        </p>
    </section>
</div>
