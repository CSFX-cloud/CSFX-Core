<script lang="ts">
    import { Bot } from "@lucide/svelte";
</script>

<section class="cx-panel nd-empty" aria-label="Tasks" style="min-height: 200px">
    <div class="nd-empty__in">
        <Bot class="cx-i nd-empty__icon" aria-hidden="true" />
        <p class="nd-empty__title nd-empty__title--sm">No active tasks.</p>
        <p class="nd-empty__text nd-empty__text--sm">Restarts, migrations and updates on this node show up here while they run.</p>
    </div>
</section>
