<script lang="ts">
    import { ChevronDown, ChevronUp, RefreshCw, Square, Trash2 } from "@lucide/svelte";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { stackStatus, type WorkloadStack } from "$lib/utils/resource-items";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    type Props = {
        stack: WorkloadStack;
        expanded: boolean;
        href: string;
        nodeName: string;
        onToggle: () => void;
        onRestart: () => void;
        onStop: () => void;
        onDelete: () => void;
    };

    let { stack, expanded, href, nodeName, onToggle, onRestart, onStop, onDelete }: Props = $props();

    const status = $derived(stackStatus(stack.children));
    const running = $derived(stack.children.filter((child) => child.status === "running").length);
    const actions = $derived<MenuAction[]>([
        { label: "Restart stack", icon: RefreshCw, onselect: onRestart },
        { label: "Stop stack", icon: Square, onselect: onStop },
        { label: "Delete stack", icon: Trash2, onselect: onDelete, danger: true },
    ]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <button class="rs-expand" type="button" aria-label={expanded ? "Collapse stack" : "Expand stack"} aria-expanded={expanded} onclick={onToggle}>
                {#if expanded}<ChevronUp class="cx-i" aria-hidden="true" />{:else}<ChevronDown class="cx-i" aria-hidden="true" />{/if}
            </button>
            <ServiceIcon name="docker-compose" />
            <a class="rs-link" {href}>{stack.stack_name}</a>
        </div>
    </td>
    <td>
        <span class={badgeClass(status)} title="{running}/{stack.children.length} running">{statusLabel(status)}</span>
    </td>
    <td class="cx-soft">{stack.children.length} services &middot; {running} running</td>
    <td class="cx-soft">{nodeName}</td>
    <td class="cx-num">{stack.children[0]?.created_at.slice(0, 10) ?? "—"}</td>
    <td><ActionMenu label="Actions for {stack.stack_name}" {actions} /></td>
</tr>
