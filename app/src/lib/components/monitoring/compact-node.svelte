<script lang="ts">
    import Icon from "@iconify/svelte";
    import { Handle, Position, type NodeProps } from "@xyflow/svelte";
    import { statusVariant } from "$lib/utils/status";

    const DOT_COLORS = { success: "var(--success)", destructive: "var(--danger)" };

    let { data, type, selected }: NodeProps = $props();

    const variant = $derived(statusVariant(String(data.status ?? "")));
    const dotColor = $derived(DOT_COLORS[variant as keyof typeof DOT_COLORS] ?? "var(--warning)");
    const subtitle = $derived(type === "workload" ? String(data.image ?? "") : type === "volume" ? `${data.size_gb} GB` : "");
</script>

<Handle type="target" position={Position.Top} />
<Handle type="target" position={Position.Left} id="left" />
<div
    class="tp-card {type === 'cluster' ? 'tp-card--cluster' : ''} {selected ? 'is-selected' : ''}"
    style={data.groupColor ? `border-left: 3px solid ${data.groupColor}` : ""}
>
    <div class="tp-card__row">
        <Icon icon={String(data.icon)} width={18} height={18} class="shrink-0" />
        <span>{data.label}</span>
        {#if type !== "cluster"}
            <span class="tp-card__dot" style="background: {dotColor}"></span>
        {/if}
    </div>
    {#if subtitle}<div class="tp-card__sub">{subtitle}</div>{/if}
</div>
<Handle type="source" position={Position.Bottom} />
<Handle type="source" position={Position.Right} id="right" />
