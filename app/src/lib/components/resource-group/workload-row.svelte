<script lang="ts">
    import Icon from "@iconify/svelte";
    import type { Workload } from "$lib/api/resource-groups";
    import { resolveImageIcon } from "$lib/utils/image-icon";
    import StatusBadge from "$lib/components/status-badge.svelte";

    let {
        w,
        indented = false,
        onOpen,
        onRestart,
        onStop,
        onDelete,
    }: {
        w: Workload;
        indented?: boolean;
        onOpen: () => void;
        onRestart: () => void;
        onStop: () => void;
        onDelete: () => void;
    } = $props();
</script>

<tr
    class="border-t hover:bg-muted/20 transition-colors cursor-pointer"
    onclick={onOpen}
>
    <td class="px-4 py-3">
        <div class="flex items-center gap-2.5" class:ps-6={indented}>
            <div class="flex w-5 shrink-0 items-center justify-center">
                <Icon icon={w.runtime_class === "vm" ? "mdi:monitor" : resolveImageIcon(w.image)} width={20} height={20} />
            </div>
            <p class="font-medium leading-tight">{w.service_name ?? w.name}</p>
        </div>
    </td>
    <td class="px-4 py-3">
        <span class="text-xs px-2 py-0.5 rounded border font-medium">{w.runtime_class === "vm" ? "VM" : "Container"}</span>
    </td>
    <td class="px-4 py-3 text-xs text-muted-foreground font-mono">
        {w.assigned_agent_id ? w.assigned_agent_id.slice(0, 8) : "-"}
    </td>
    <td class="px-4 py-3">
        <div class="flex items-center gap-2 min-w-[120px]">
            <span class="text-xs text-muted-foreground w-8">{w.cpu_millicores}m</span>
            <div class="flex-1 h-1 rounded-full bg-muted overflow-hidden">
                <div class="h-full rounded-full bg-foreground/60" style="width: {Math.min(100, w.cpu_millicores / 10)}%"></div>
            </div>
        </div>
    </td>
    <td class="px-4 py-3">
        <div class="flex items-center gap-1.5">
            <StatusBadge status={w.status} />
            {#if w.restart_count > 0}
                <span
                    class="text-xs px-1.5 py-0.5 rounded-full font-medium bg-amber-500/10 text-amber-600"
                    title="Restarted {w.restart_count} time{w.restart_count === 1 ? '' : 's'}{w.max_restarts !== null ? ` (max ${w.max_restarts})` : ''}"
                >
                    ↻ {w.restart_count}
                </span>
            {/if}
        </div>
    </td>
    <td class="px-4 py-3">
        <div class="flex items-center justify-end gap-1">
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={(e) => { e.stopPropagation(); onRestart(); }}
                aria-label="Restart"
                title="Restart"
            >
                <Icon icon="mdi:restart" width={16} height={16} />
            </button>
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-40 disabled:pointer-events-none"
                onclick={(e) => { e.stopPropagation(); onStop(); }}
                disabled={w.desired_state === "stopped"}
                aria-label="Stop"
                title="Stop"
            >
                <Icon icon="mdi:stop-circle-outline" width={16} height={16} />
            </button>
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-destructive hover:bg-destructive/10 transition-colors"
                onclick={(e) => { e.stopPropagation(); onDelete(); }}
                aria-label="Delete"
                title="Delete"
            >
                <Icon icon="mdi:trash-can-outline" width={16} height={16} />
            </button>
        </div>
    </td>
</tr>
