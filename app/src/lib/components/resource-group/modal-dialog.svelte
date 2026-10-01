<script lang="ts">
    import type { Snippet } from "svelte";
    import Icon from "@iconify/svelte";

    let {
        title,
        subtitle,
        width = "max-w-sm",
        onclose,
        children,
    }: {
        title: string;
        subtitle?: string;
        width?: string;
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

<dialog
    bind:this={element}
    class="fixed inset-0 z-50 m-auto w-full {width} rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    {onclose}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <div>
                <h2 class="text-base font-semibold">{title}</h2>
                {#if subtitle}
                    <p class="text-xs text-muted-foreground font-mono">{subtitle}</p>
                {/if}
            </div>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={close}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        {@render children()}
    </div>
</dialog>
