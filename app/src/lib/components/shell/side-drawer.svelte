<script lang="ts">
    import type { Snippet } from "svelte";
    import { X } from "@lucide/svelte";

    type Props = { open: boolean; title: string; subtitle?: string; wide?: boolean; onclose: () => void; header?: Snippet; footer?: Snippet; children: Snippet };

    let { open, title, subtitle, wide = false, onclose, header, footer, children }: Props = $props();

    let element = $state<HTMLDialogElement | null>(null);

    $effect(() => {
        if (!element) return;
        if (open && !element.open) element.showModal();
        if (!open && element.open) element.close();
    });
</script>

<dialog bind:this={element} class="md-dialog md-drawer" class:md-drawer--wide={wide} class:md-drawer--split={footer !== undefined} {onclose} onclick={(event) => event.target === element && onclose()}>
    <div class="md__body">
        <div class="md__head">
            <div class="st-split" style="justify-content: flex-start; gap: 12px; min-width: 0">
                {@render header?.()}
                <div style="min-width: 0">
                    <h2 style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{title}</h2>
                    {#if subtitle}<p class="cx-mono" style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{subtitle}</p>{/if}
                </div>
            </div>
            <button class="cx-icon-btn cx-icon-btn--sm" type="button" tabindex="-1" aria-label="Close" onclick={onclose}>
                <X class="cx-i" aria-hidden="true" />
            </button>
        </div>
        {#if footer}
            <div class="md__scroll">{@render children()}</div>
            <div class="md__foot">{@render footer()}</div>
        {:else}
            {@render children()}
        {/if}
    </div>
</dialog>
