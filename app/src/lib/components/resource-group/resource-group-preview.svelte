<script lang="ts">
    import Icon from "@iconify/svelte";
    import type { ResourceGroup } from "$lib/api/resource-groups";
    import RgIcon from "$lib/components/rg-icon.svelte";

    const FRAME_WIDTH = 1280;
    const FRAME_HEIGHT = 800;

    let { group, onSave }: { group: ResourceGroup; onSave: (url: string) => void } = $props();

    let editing = $state(false);
    let draft = $state("");
    let width = $state(0);

    const scale = $derived(width / FRAME_WIDTH);

    function startEdit() {
        draft = group.preview_url ?? "";
        editing = true;
    }

    function commit() {
        if (!editing) return;
        editing = false;
        if (draft.trim() !== (group.preview_url ?? "")) onSave(draft);
    }

    function handleKeydown(event: KeyboardEvent) {
        if (event.key === "Enter") commit();
        if (event.key === "Escape") editing = false;
    }

    function focusOnMount(node: HTMLInputElement) {
        node.focus();
        node.select();
    }
</script>

<div class="flex flex-col gap-2">
    <div
        class="relative w-full overflow-hidden rounded-lg border bg-muted/40"
        style="aspect-ratio: {FRAME_WIDTH} / {FRAME_HEIGHT};"
        bind:clientWidth={width}
    >
        {#if group.preview_url}
            <iframe
                src={group.preview_url}
                title="{group.name} preview"
                class="absolute left-0 top-0 origin-top-left border-0 pointer-events-none bg-white"
                style="width: {FRAME_WIDTH}px; height: {FRAME_HEIGHT}px; transform: scale({scale});"
                loading="lazy"
                sandbox="allow-scripts allow-same-origin"
            ></iframe>
            {#if !editing}
                <button
                    class="absolute right-1.5 top-1.5 flex h-7 w-7 items-center justify-center rounded-full bg-background/80 text-muted-foreground hover:text-foreground transition-colors"
                    onclick={startEdit}
                    aria-label="Edit preview"
                    title="Edit preview"
                >
                    <Icon icon="mdi:pencil-outline" width={14} height={14} />
                </button>
            {/if}
        {:else}
            <button
                class="absolute inset-0 flex flex-col items-center justify-center gap-3 animate-pulse hover:animate-none"
                onclick={startEdit}
                aria-label="Set preview"
                title="Set preview"
            >
                <div class="flex h-12 w-12 items-center justify-center rounded-full bg-background/60">
                    <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={24} />
                </div>
                <div class="h-2 w-20 rounded-full bg-foreground/10"></div>
                <div class="h-2 w-12 rounded-full bg-foreground/10"></div>
            </button>
        {/if}
    </div>
    {#if editing}
        <input
            class="w-full border rounded px-2 py-1 text-xs bg-background font-mono"
            placeholder="https://10.101.0.5:8080 (empty to clear)"
            bind:value={draft}
            onblur={commit}
            onkeydown={handleKeydown}
            use:focusOnMount
        />
    {/if}
</div>
