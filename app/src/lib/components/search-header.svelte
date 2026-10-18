<script lang="ts">
    import BellIcon from "@lucide/svelte/icons/bell";
    import { Button } from "$lib/components/ui/button/index.js";
    import SearchTrigger from "$lib/components/search-trigger.svelte";
</script>

<header class="flex h-16 shrink-0 items-center gap-3 px-4">
    <SearchTrigger />
    <div class="flex items-center gap-1">
        <Button variant="ghost" size="icon-sm" aria-label="Notifications">
            <BellIcon class="size-4" />
        </Button>
    </div>
</header>
