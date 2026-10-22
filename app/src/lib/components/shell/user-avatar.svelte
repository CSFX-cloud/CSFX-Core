<script lang="ts">
    import { Blobatar } from "@blobatar/svelte";
    import { gaze } from "@blobatar/svelte/gaze";
    import "blobatar/motion.css";
    import "blobatar/gaze.css";
    import { auth } from "$lib/auth/store.svelte";
    import { gravatarUrl, getAvatarFallbackStyle, type AvatarFallbackStyle } from "$lib/auth/api";

    const AVATAR_SIZE = 22;
    const INITIALS_LENGTH = 2;

    let avatarUrl = $state<string | null>(null);
    let fallbackStyle = $state<AvatarFallbackStyle>("initials");

    const eyes = gaze({ travel: 3, target: "pointer" });

    $effect(() => {
        const source = auth.user?.gravatar_email;
        if (!source) {
            avatarUrl = null;
            return;
        }
        gravatarUrl(source, AVATAR_SIZE * 2).then((url) => (avatarUrl = url));
    });

    $effect(() => {
        if (!auth.token) return;
        getAvatarFallbackStyle(auth.token)
            .then((style) => (fallbackStyle = style))
            .catch(() => (fallbackStyle = "initials"));
    });
</script>

<span class="db-avatar" aria-hidden="true">
    {#if avatarUrl}
        <img class="db-avatar__img" src={avatarUrl} alt="" />
    {:else if fallbackStyle === "blobatar" && auth.user}
        <Blobatar {@attach eyes} name={auth.user.username} size={AVATAR_SIZE} class="db-avatar__blob" animate="always" />
    {:else}
        {auth.user ? auth.user.username.slice(0, INITIALS_LENGTH).toUpperCase() : "??"}
    {/if}
</span>
