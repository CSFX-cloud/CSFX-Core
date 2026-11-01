<script lang="ts">
    import { ExternalLink, Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import type { Registry } from "$lib/api/registries";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";

    let { r, href, onDelete }: { r: Registry; href: string; onDelete: () => void } = $props();

    const actions = $derived<MenuAction[]>([
        { label: "Open", icon: ExternalLink, onselect: () => goto(href) },
        { label: "Delete", icon: Trash2, onselect: onDelete, danger: true },
    ]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <ServiceIcon name="container-registry" />
            <span class="rs-text">
                <button class="rs-link" type="button" onclick={() => goto(href)}>{r.name}</button>
                <span>Container Registry</span>
            </span>
        </div>
    </td>
    <td><span class="cx-badge cx-badge--success">Active</span></td>
    <td class="cx-soft">{r.prefix}</td>
    <td class="cx-soft">&mdash;</td>
    <td class="cx-num">{r.created_at.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {r.name}" {actions} /></td>
</tr>
