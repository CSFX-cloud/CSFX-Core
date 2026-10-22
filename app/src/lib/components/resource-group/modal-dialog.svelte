<script lang="ts">
    import type { Snippet } from "svelte";
    import { X } from "@lucide/svelte";

    type Size = "sm" | "md" | "lg" | "xl";

    let {
        title,
        subtitle,
        size = "sm",
        onclose,
        children,
    }: {
        title: string;
        subtitle?: string;
        size?: Size;
        onclose?: () => void;
        children: Snippet;
    } = $props();

    let element = $state<HTMLDialogElement | null>(null);

    export function open() {
        element?.showModal();
    }

    export function close() {
        element?.close();
    }
</script>

<dialog bind:this={element} class="md-dialog {size === 'sm' ? '' : `md-dialog--${size}`}" {onclose}>
    <div class="md__body">
        <div class="md__head">
            <div>
                <h2>{title}</h2>
                {#if subtitle}<p class="cx-mono">{subtitle}</p>{/if}
            </div>
            <button class="cx-icon-btn cx-icon-btn--sm" type="button" tabindex="-1" aria-label="Close" onclick={close}>
                <X class="cx-i" aria-hidden="true" />
            </button>
        </div>
        {@render children()}
    </div>
</dialog>
