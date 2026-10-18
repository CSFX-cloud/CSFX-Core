<script lang="ts">
    import Icon from "@iconify/svelte";
    import { Handle, Position, type NodeProps } from "@xyflow/svelte";
    import { statusVariant } from "$lib/utils/status";

    let { data, type, selected }: NodeProps = $props();

    const status = $derived(String(data.status ?? ""));
    const variant = $derived(statusVariant(status));
    const dotClass = $derived(
        variant === "success" ? "bg-green-500" : variant === "destructive" ? "bg-red-500" : "bg-yellow-500",
    );
    const subtitle = $derived(
        type === "workload"
            ? String(data.image ?? "")
            : type === "volume"
              ? `${data.size_gb} GB`
              : "",
    );
</script>

<Handle type="target" position={Position.Top} />
<Handle type="target" position={Position.Left} id="left" />
<div
    class="rounded-md border bg-card px-3 py-1.5 text-card-foreground shadow-sm {type === 'cluster' ? 'w-48 font-semibold' : 'w-44'} {selected ? 'ring-2 ring-primary' : ''}"
    style={data.groupColor ? `border-left: 4px solid ${data.groupColor}` : ""}
>
    <div class="flex items-center gap-2">
        <Icon icon={String(data.icon)} width={20} height={20} class="shrink-0" />
        <span class="truncate text-sm">{data.label}</span>
        {#if type !== "cluster"}
            <span class="ms-auto size-2 shrink-0 rounded-full {dotClass}"></span>
        {/if}
    </div>
    {#if subtitle}
        <div class="truncate text-[11px] text-muted-foreground">{subtitle}</div>
    {/if}
</div>
<Handle type="source" position={Position.Bottom} />
<Handle type="source" position={Position.Right} id="right" />
