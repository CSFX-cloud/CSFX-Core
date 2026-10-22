<script lang="ts">
    import { Trash2 } from "@lucide/svelte";
    import type { Volume } from "$lib/api/resource-groups";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    let { v, onDelete }: { v: Volume; onDelete: () => void } = $props();

    const actions = $derived<MenuAction[]>([{ label: "Delete", icon: Trash2, onselect: onDelete, danger: true, disabled: v.status === "in_use" }]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <ServiceIcon name="block-storage" />
            <span class="rs-text"><b>{v.name}</b></span>
        </div>
    </td>
    <td><span class={badgeClass(v.status)}>{statusLabel(v.status)}</span></td>
    <td class="cx-soft">{v.size_gb} GB &middot; {v.pool}</td>
    <td class="cx-soft">&mdash;</td>
    <td class="cx-num">{v.created_at.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {v.name}" {actions} /></td>
</tr>
