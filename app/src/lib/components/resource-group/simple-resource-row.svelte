<script lang="ts">
    import type { Component } from "svelte";
    import { Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";

    type Props = {
        name: string;
        kind: string;
        service: string;
        openIcon: Component;
        href: string;
        badgeClass: string;
        badgeLabel: string;
        detail: string;
        createdAt: string;
        onDelete: () => void;
    };

    let { name, kind, service, openIcon, href, badgeClass, badgeLabel, detail, createdAt, onDelete }: Props = $props();

    const actions = $derived<MenuAction[]>([
        { label: "Open", icon: openIcon, onselect: () => goto(href) },
        { label: "Delete", icon: Trash2, onselect: onDelete, danger: true },
    ]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <ServiceIcon name={service} />
            <span class="rs-text">
                <button class="rs-link" type="button" onclick={() => goto(href)}>{name}</button>
                <span>{kind}</span>
            </span>
        </div>
    </td>
    <td><span class="cx-badge {badgeClass}">{badgeLabel}</span></td>
    <td class="cx-soft">{detail}</td>
    <td class="cx-soft">&mdash;</td>
    <td class="cx-num">{createdAt.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {name}" {actions} /></td>
</tr>
