<script lang="ts">
    import BellIcon from "@lucide/svelte/icons/bell";
    import SearchIcon from "@lucide/svelte/icons/search";
    import { Button } from "$lib/components/ui/button/index.js";
    import { commandPalette } from "$lib/components/command-palette/command-palette-store.svelte.js";
</script>

<header class="flex h-16 shrink-0 items-center gap-3 px-4">
    <div class="flex-1 flex justify-center">
        <button type="button" onclick={() => commandPalette.show()} class="relative w-full max-w-sm text-left">
            <SearchIcon class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <span class="flex items-center h-9 w-full rounded-md border border-input bg-background pl-8 pr-14 text-sm text-muted-foreground hover:bg-accent/50 transition-colors">
                Search anything
            </span>
            <kbd class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 rounded border border-border bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                &#8984;K
            </kbd>
        </button>
    </div>
    <div class="flex items-center gap-1">
        <Button variant="ghost" size="icon-sm" aria-label="Notifications">
            <BellIcon class="size-4" />
        </Button>
    </div>
</header>
