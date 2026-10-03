<script lang="ts">
    import type { Volume } from "$lib/api/resource-groups";
    import Icon from "@iconify/svelte";
    import StatusBadge from "$lib/components/status-badge.svelte";

    let { v, onDelete }: { v: Volume; onDelete: () => void } = $props();
</script>

<tr class="border-t hover:bg-muted/20 transition-colors">
    <td class="px-4 py-3">
        <div class="flex items-center gap-2.5">
            <div class="flex h-7 w-7 shrink-0 items-center justify-center rounded border bg-muted/50">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
            </div>
            <div>
                <p class="font-medium leading-tight">{v.name}</p>
                <p class="text-xs text-muted-foreground">{v.size_gb} GB · {v.pool}</p>
            </div>
        </div>
    </td>
    <td class="px-4 py-3">
        <span class="text-xs px-2 py-0.5 rounded border font-medium">Volume</span>
    </td>
    <td class="px-4 py-3 text-xs text-muted-foreground">
        {v.size_gb} GB
    </td>
    <td class="px-4 py-3">
        <StatusBadge status={v.status} />
    </td>
    <td class="px-4 py-3 text-right">
        <button
            class="inline-flex items-center justify-center w-7 h-7 rounded-full text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-40 disabled:pointer-events-none"
            onclick={() => onDelete()}
            disabled={v.status === "in_use"}
            aria-label="Delete"
            title="Delete"
        >
            <Icon icon="mdi:trash-can-outline" width={16} height={16} />
        </button>
    </td>
</tr>
