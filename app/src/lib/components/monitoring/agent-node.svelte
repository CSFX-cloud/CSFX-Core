<script lang="ts">
    import { Server } from "@lucide/svelte";
    import { Handle, Position, type NodeProps } from "@xyflow/svelte";
    import { statusVariant } from "$lib/utils/status";
    import { nodeTone } from "$lib/utils/dashboard";

    let { data, selected }: NodeProps = $props();

    const status = $derived(String(data.status ?? "unknown"));
    const online = $derived(statusVariant(status) === "success");
    const statusText = $derived(status.charAt(0).toUpperCase() + status.slice(1).toLowerCase());

    function percent(value: unknown): string {
        return typeof value === "number" ? `${value.toFixed(1)}%` : "—";
    }
</script>

<Handle type="target" position={Position.Top} />
<div class="tp-node {online ? '' : 'tp-node--off'} {selected ? 'is-selected' : ''} {data.cordoned ? 'tp-node--off' : ''}">
    <span class="tp-node__top">
        <span class="{online ? 'cx-tile-glow' : 'cx-tile-plain'} nl-tile" aria-hidden="true"><Server class="cx-i" /></span>
        <b>{data.label}</b>
        <span class="cx-badge cx-badge--{nodeTone(status)}">{statusText}</span>
    </span>
    <span class="tp-node__ip">{data.ip_address ?? data.hostname} &middot; {data.role}</span>
    <span class="tp-node__metrics">
        <span>CPU<b>{percent(data.cpu_percent)}</b></span>
        <span>RAM<b>{percent(data.memory_percent)}</b></span>
        <span>Disk<b>{percent(data.disk_percent)}</b></span>
    </span>
</div>
<Handle type="source" position={Position.Bottom} />
