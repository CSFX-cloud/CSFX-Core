<script lang="ts">
    import { FolderOpen, Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import type { Bucket } from "$lib/api/resource-groups";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { fmtBytes } from "$lib/utils/format";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    let { b, onOpen, onDelete }: { b: Bucket; onOpen: () => void; onDelete: () => void } = $props();

    const actions = $derived<MenuAction[]>([
        { label: "Browse", icon: FolderOpen, onselect: () => goto(`/buckets/${b.id}`) },
        { label: "Delete", icon: Trash2, onselect: onDelete, danger: true },
    ]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <ServiceIcon name="object-storage" />
            <span class="rs-text">
                <button class="rs-link" type="button" onclick={onOpen}>{b.name}</button>
                <span>{b.global_alias}</span>
            </span>
        </div>
    </td>
    <td><span class={badgeClass(b.status)}>{statusLabel(b.status)}</span></td>
    <td class="cx-soft">{b.quota_max_size ? fmtBytes(b.quota_max_size) : "unlimited"} &middot; {b.exposure}</td>
    <td class="cx-soft">&mdash;</td>
    <td class="cx-num">{b.created_at.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {b.name}" {actions} /></td>
</tr>
