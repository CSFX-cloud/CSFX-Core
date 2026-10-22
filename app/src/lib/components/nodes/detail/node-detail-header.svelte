<script lang="ts">
    import { ArrowLeft, ArrowRight, Ban, Clock, Globe, Power, RefreshCw, Server, Tag, Trash2, Undo2, Wrench } from "@lucide/svelte";
    import type { Node } from "$lib/api/nodes";
    import type { NodeActions } from "$lib/components/nodes/detail/types";
    import { nodeTone } from "$lib/utils/dashboard";
    import { formatUptime } from "$lib/utils/format";

    type Props = {
        node: Node;
        uptimeSeconds: number | null;
        maintenanceRemaining: string | null;
        inMaintenance: boolean;
        busy: boolean;
        actions: NodeActions;
    };

    let { node, uptimeSeconds, maintenanceRemaining, inMaintenance, busy, actions }: Props = $props();

    const statusLabel = $derived(node.status.charAt(0).toUpperCase() + node.status.slice(1).toLowerCase());
</script>

<section class="cx-stage nd-hero" aria-label="Node">
    <div class="cx-stage__canvas"></div>
    <div class="nd-head nd-hero-in">
        <div class="nd-id">
            <a class="cx-icon-btn" href="/nodes" aria-label="Back to nodes" title="Back to nodes">
                <ArrowLeft class="cx-i" aria-hidden="true" />
            </a>
            <span class="nd-tile" aria-hidden="true"><Server class="cx-i" /></span>
            <div style="min-width: 0">
                <div class="nd-title">
                    <h1>{node.hostname}</h1>
                    <span class="cx-badge cx-badge--{nodeTone(node.status)}">{statusLabel}</span>
                    <span class="cx-badge">{node.role}</span>
                    {#if node.cordoned}<span class="cx-badge cx-badge--warning">Cordoned</span>{/if}
                    {#if inMaintenance}
                        <span class="cx-badge cx-badge--warning">Maintenance{maintenanceRemaining ? ` · ${maintenanceRemaining}` : ""}</span>
                    {/if}
                </div>
                <div class="nd-meta">
                    <span><Globe class="cx-i" aria-hidden="true" />{node.ip_address ?? "no ip"}</span>
                    <span><Tag class="cx-i" aria-hidden="true" />v{node.agent_version}</span>
                    {#if uptimeSeconds != null}
                        <span><Clock class="cx-i" aria-hidden="true" />Up {formatUptime(uptimeSeconds)}</span>
                    {/if}
                </div>
            </div>
        </div>
        <div class="db-actions nd-actions">
            {#if inMaintenance}
                <button class="cx-icon-btn nd-warn" type="button" disabled={busy} aria-label="End maintenance" title="End maintenance" onclick={actions.endMaintenance}>
                    <Wrench class="cx-i" aria-hidden="true" />
                </button>
            {:else}
                <button class="cx-icon-btn" type="button" aria-label="Start maintenance" title="Start maintenance" onclick={actions.startMaintenance}>
                    <Wrench class="cx-i" aria-hidden="true" />
                </button>
            {/if}
            {#if node.cordoned}
                <button class="cx-icon-btn" type="button" disabled={busy} aria-label="Uncordon node" title="Uncordon node" onclick={actions.uncordon}>
                    <Undo2 class="cx-i" aria-hidden="true" />
                </button>
                <button class="cx-icon-btn nd-danger" type="button" disabled={busy} aria-label="Remove from cluster" title="Remove from cluster" onclick={actions.remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            {:else}
                <button class="cx-icon-btn" type="button" disabled={busy} aria-label="Cordon node" title="Cordon node" onclick={actions.cordon}>
                    <Ban class="cx-i" aria-hidden="true" />
                </button>
                <button class="cx-icon-btn" type="button" disabled={busy} aria-label="Drain workloads" title="Drain workloads" onclick={actions.drain}>
                    <ArrowRight class="cx-i" aria-hidden="true" />
                </button>
            {/if}
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={busy} onclick={actions.reboot}>
                <RefreshCw class="cx-i" aria-hidden="true" />
                Restart
            </button>
            <button class="cx-btn cx-btn--secondary cx-btn--sm nd-danger" type="button" disabled={busy} onclick={actions.powerOff}>
                <Power class="cx-i" aria-hidden="true" />
                Power off
            </button>
        </div>
    </div>
</section>
