<script lang="ts">
    import { gravatarUrl } from "$lib/auth/api";

    const INITIALS_LENGTH = 2;
    const FONT_RATIO = 0.4;
    const RETINA = 2;

    let { name, gravatarEmail, size = 30 }: { name: string; gravatarEmail: string | null; size?: number } = $props();

    let url = $state<string | null>(null);

    $effect(() => {
        if (!gravatarEmail) {
            url = null;
            return;
        }
        gravatarUrl(gravatarEmail, size * RETINA).then((resolved) => (url = resolved));
    });
</script>

<span class="st-avatar" style="width: {size}px; height: {size}px; font-size: {Math.round(size * FONT_RATIO)}px" aria-hidden="true">
    {#if url}<img src={url} alt="" />{:else}{name.slice(0, INITIALS_LENGTH).toUpperCase()}{/if}
</span>
