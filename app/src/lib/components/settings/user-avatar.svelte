<script lang="ts">
    import * as Avatar from "$lib/components/ui/avatar/index.js";
    import { gravatarUrl } from "$lib/auth/api";

    let { name, gravatarEmail }: { name: string; gravatarEmail: string | null } = $props();

    let url = $state<string | null>(null);

    $effect(() => {
        if (!gravatarEmail) {
            url = null;
            return;
        }
        gravatarUrl(gravatarEmail, 64).then((resolved) => (url = resolved));
    });
</script>

<Avatar.Root class="size-8 rounded-lg">
    {#if url}
        <Avatar.Image src={url} alt={name} />
    {/if}
    <Avatar.Fallback class="rounded-lg text-xs">{name.slice(0, 2).toUpperCase()}</Avatar.Fallback>
</Avatar.Root>
