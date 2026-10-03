<script lang="ts">
    import Icon from "@iconify/svelte";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import { stackStatus, type WorkloadStack } from "$lib/utils/resource-items";

    let {
        stack,
        expanded,
        onOpen,
        onToggle,
        onRestart,
        onStop,
        onDelete,
    }: {
        stack: WorkloadStack;
        expanded: boolean;
        onOpen: () => void;
        onToggle: () => void;
        onRestart: () => void;
        onStop: () => void;
        onDelete: () => void;
    } = $props();
</script>

<tr
    class="border-t hover:bg-muted/20 transition-colors cursor-pointer"
    onclick={() => onOpen()}
>
    <td class="px-4 py-3">
        <div class="flex items-center gap-2.5">
            <button
                class="flex items-center justify-center w-5 h-5 shrink-0 rounded hover:bg-muted transition-colors"
                onclick={(e) => { e.stopPropagation(); onToggle(); }}
                aria-label={expanded ? "Collapse" : "Expand"}
                title={expanded ? "Collapse" : "Expand"}
            >
                <Icon icon={expanded ? "mdi:chevron-up" : "mdi:chevron-down"} width={16} height={16} class="text-muted-foreground" />
            </button>
            <div class="flex w-5 shrink-0 items-center justify-center">
                <Icon icon="logos:docker-icon" width={20} height={20} />
            </div>
            <div>
                <p class="font-medium leading-tight">{stack.stack_name}</p>
                <p class="text-xs text-muted-foreground">{stack.children.length} services</p>
            </div>
        </div>
    </td>
    <td class="px-4 py-3">
        <span class="text-xs px-2 py-0.5 rounded border font-medium">Compose Stack</span>
    </td>
    <td class="px-4 py-3 text-xs text-muted-foreground font-mono">
        {stack.children[0]?.assigned_agent_id ? stack.children[0].assigned_agent_id!.slice(0, 8) : "-"}
    </td>
    <td class="px-4 py-3">
        <StatusBadge
            status={stackStatus(stack.children)}
            label="{stack.children.filter((c) => c.status === 'running').length}/{stack.children.length} running"
        />
    </td>
    <td class="px-4 py-3">
        <div class="flex items-center justify-end gap-1">
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={(e) => { e.stopPropagation(); onRestart(); }}
                aria-label="Restart stack"
                title="Restart stack"
            >
                <Icon icon="mdi:restart" width={16} height={16} />
            </button>
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={(e) => { e.stopPropagation(); onStop(); }}
                aria-label="Stop stack"
                title="Stop stack"
            >
                <Icon icon="mdi:stop-circle-outline" width={16} height={16} />
            </button>
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-destructive hover:bg-destructive/10 transition-colors"
                onclick={(e) => { e.stopPropagation(); onDelete(); }}
                aria-label="Delete stack"
                title="Delete stack"
            >
                <Icon icon="mdi:trash-can-outline" width={16} height={16} />
            </button>
        </div>
    </td>
</tr>
