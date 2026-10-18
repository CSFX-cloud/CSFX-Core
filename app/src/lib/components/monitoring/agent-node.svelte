<script lang="ts">
    import { Handle, Position, type NodeProps } from "@xyflow/svelte";
    import { statusVariant } from "$lib/utils/status";

    let { data, selected }: NodeProps = $props();

    const status = $derived(String(data.status ?? "unknown"));
    const online = $derived(statusVariant(status) === "success");

    function percent(value: unknown): string {
        return typeof value === "number" ? `${value.toFixed(1)}%` : "-";
    }
</script>

<Handle type="target" position={Position.Top} />
<div
    class="w-60 rounded-lg border bg-card px-3 py-2 text-card-foreground shadow-sm {selected ? 'ring-2 ring-primary' : ''} {data.cordoned ? 'opacity-70' : ''}"
>
    <div class="flex items-center gap-2">
        <span class="size-2 rounded-full {online ? 'bg-green-500' : 'bg-red-500'}"></span>
        <span class="truncate text-sm font-semibold">{data.label}</span>
        <span class="ms-auto rounded bg-muted px-1.5 py-0.5 text-[10px] uppercase text-muted-foreground">{data.role}</span>
    </div>
    <div class="mt-1 truncate text-xs text-muted-foreground">{data.ip_address ?? data.hostname}</div>
    <div class="mt-2 flex gap-3 text-xs">
        <span title="CPU">CPU {percent(data.cpu_percent)}</span>
        <span title="Memory">RAM {percent(data.memory_percent)}</span>
        <span title="Disk">Disk {percent(data.disk_percent)}</span>
    </div>
</div>
<Handle type="source" position={Position.Bottom} />
