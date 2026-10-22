<script lang="ts">
    import { gravatarUrl } from "$lib/auth/api";

    const AVATAR_SIZE = 30;
    const INITIALS_LENGTH = 2;

    let { name, gravatarEmail }: { name: string; gravatarEmail: string | null } = $props();

    let url = $state<string | null>(null);

    $effect(() => {
        if (!gravatarEmail) {
            url = null;
            return;
        }
        gravatarUrl(gravatarEmail, AVATAR_SIZE * 2).then((resolved) => (url = resolved));
    });
</script>

<span class="st-avatar" style="width: {AVATAR_SIZE}px; height: {AVATAR_SIZE}px; font-size: 11px" aria-hidden="true">
    {#if url}<img src={url} alt="" />{:else}{name.slice(0, INITIALS_LENGTH).toUpperCase()}{/if}
</span>
