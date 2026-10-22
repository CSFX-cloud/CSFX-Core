<script lang="ts">
    import type { Snippet } from "svelte";
    import { ChevronLeft } from "@lucide/svelte";

    let { backHref, backLabel, actions }: { backHref: string; backLabel: string; actions?: Snippet } = $props();
</script>

<div class="rg-bar">
    <a class="ga-back" href={backHref}>
        <ChevronLeft class="cx-i" aria-hidden="true" />
        {backLabel}
    </a>
    {#if actions}<div class="db-actions">{@render actions()}</div>{/if}
</div>
