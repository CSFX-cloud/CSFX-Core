<script lang="ts">
    import { levelBarClass, levelBars } from "$lib/utils/logs";

    let { level }: { level: string } = $props();
</script>

<div class="flex gap-0.5" title={level}>
    {#each [1, 2, 3] as bar (bar)}
        <span class="h-2.5 w-1.5 rounded-sm {bar <= levelBars(level) ? levelBarClass(level) : 'bg-muted'}"></span>
    {/each}
</div>
