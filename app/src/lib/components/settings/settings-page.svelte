<script lang="ts">
    import type { Snippet } from "svelte";

    type Props = { title: string; description: string; actions?: Snippet; children: Snippet };

    let { title, description, actions, children }: Props = $props();
</script>

<div class="st-wrap">
    <div class="st-head">
        <div>
            <h2>{title}</h2>
            <p>{description}</p>
        </div>
        {#if actions}<div class="db-actions">{@render actions()}</div>{/if}
    </div>
    {@render children()}
</div>
