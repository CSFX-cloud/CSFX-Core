<script lang="ts">
    import type { Server } from "@lucide/svelte";

    let { icon: Icon, label }: { icon: typeof Server; label: string } = $props();
</script>

<div class="db-empty-state">
    <span class="db-empty-state__icon"><Icon class="cx-i" aria-hidden="true" /></span>
    <span>{label}</span>
</div>
