<script lang="ts">
    import Icon from "@iconify/svelte";
    import { goto } from "$app/navigation";
    import type { Bucket } from "$lib/api/resource-groups";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import { fmtBytes } from "$lib/utils/format";

    let { b, onOpen, onDelete }: { b: Bucket; onOpen: () => void; onDelete: () => void } = $props();
</script>

<tr
    class="border-t hover:bg-muted/20 transition-colors cursor-pointer"
    onclick={() => onOpen()}
>
    <td class="px-4 py-3">
        <div class="flex items-center gap-2.5">
            <Icon icon="fluent-emoji-high-contrast:bucket" width={20} height={20} class="shrink-0" />
            <div>
                <p class="font-medium leading-tight">{b.name}</p>
                <p class="text-xs text-muted-foreground font-mono">{b.global_alias}</p>
            </div>
        </div>
    </td>
    <td class="px-4 py-3">
        <span class="text-xs px-2 py-0.5 rounded border font-medium">Bucket</span>
    </td>
    <td class="px-4 py-3 text-xs text-muted-foreground">
        {b.quota_max_size ? fmtBytes(b.quota_max_size) : "unlimited"} · {b.exposure}
    </td>
    <td class="px-4 py-3">
        <StatusBadge status={b.status} />
    </td>
    <td class="px-4 py-3 text-right">
        <div class="flex items-center justify-end gap-1">
            <button
                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={(e) => { e.stopPropagation(); goto(`/buckets/${b.id}`); }}
                aria-label="Browse"
                title="Browse"
            >
                <Icon icon="mdi:folder-open-outline" width={16} height={16} />
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
