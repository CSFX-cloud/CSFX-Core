<script lang="ts">
    import type { Snippet } from "svelte";

    let { label, value, total, foot }: { label: string; value: string; total?: string; foot: Snippet } = $props();
</script>

<div class="cx-stat">
    <p class="cx-stat__label">{label}</p>
    <p class="cx-stat__value stat">{value}{#if total} <small>/ {total}</small>{/if}</p>
    <div class="cx-stat__foot">{@render foot()}</div>
</div>
