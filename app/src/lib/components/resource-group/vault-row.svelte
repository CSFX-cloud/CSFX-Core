<script lang="ts">
    import { KeyRound, Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import type { Vault } from "$lib/api/vaults";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";

    let { v, href, onDelete }: { v: Vault; href: string; onDelete: () => void } = $props();

    const actions = $derived<MenuAction[]>([
        { label: "Open", icon: KeyRound, onselect: () => goto(href) },
        { label: "Delete", icon: Trash2, onselect: onDelete, danger: true },
    ]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <ServiceIcon name="key-vault" />
            <span class="rs-text">
                <button class="rs-link" type="button" onclick={() => goto(href)}>{v.name}</button>
                <span>Key Vault</span>
            </span>
        </div>
    </td>
    <td>
        <span class="cx-badge {v.sealed ? 'cx-badge--warning' : 'cx-badge--success'}">{v.sealed ? "Sealed" : "Unsealed"}</span>
    </td>
    <td class="cx-soft">Soft delete {v.soft_delete_days} days</td>
    <td class="cx-soft">&mdash;</td>
    <td class="cx-num">{v.created_at.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {v.name}" {actions} /></td>
</tr>
