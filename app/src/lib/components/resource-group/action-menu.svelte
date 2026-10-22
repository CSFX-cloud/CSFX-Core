<script lang="ts">
    import { Ellipsis } from "@lucide/svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";

    const MENU_GAP_PX = 4;

    let { label, actions }: { label: string; actions: MenuAction[] } = $props();

    let open = $state(false);
    let trigger = $state<HTMLButtonElement>();
    let position = $state({ top: 0, right: 0 });

    function toggle() {
        if (open) {
            open = false;
            return;
        }
        const bounds = trigger?.getBoundingClientRect();
        if (bounds) position = { top: bounds.bottom + MENU_GAP_PX, right: window.innerWidth - bounds.right };
        open = true;
    }

    function select(action: MenuAction) {
        open = false;
        action.onselect();
    }

    function closeOnOutside(event: MouseEvent) {
        if (open && !trigger?.contains(event.target as Node)) open = false;
    }

    function closeOnKey(event: KeyboardEvent) {
        if (event.key === "Escape") open = false;
    }
</script>

<svelte:window onclick={closeOnOutside} onkeydown={closeOnKey} onscroll={() => (open = false)} onresize={() => (open = false)} />

<button class="cx-icon-btn cx-icon-btn--sm rs-more" type="button" aria-label={label} aria-haspopup="menu" aria-expanded={open} bind:this={trigger} onclick={toggle}>
    <Ellipsis class="cx-i" aria-hidden="true" />
</button>

{#if open}
    <div class="am-menu" role="menu" aria-label={label} style="top: {position.top}px; right: {position.right}px">
        {#each actions as action (action.label)}
            <button class="am-item {action.danger ? 'am-item--danger' : ''}" type="button" role="menuitem" disabled={action.disabled} onclick={() => select(action)}>
                <action.icon class="cx-i" aria-hidden="true" />{action.label}
            </button>
        {/each}
    </div>
{/if}
