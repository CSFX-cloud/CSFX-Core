<script lang="ts">
    import { onMount } from "svelte";
    import * as Card from "$lib/components/ui/card/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { auth } from "$lib/auth/store.svelte";
    import { getAvatarFallbackStyle, setAvatarFallbackStyle, type AvatarFallbackStyle } from "$lib/auth/api";
    import { Blobatar } from "@blobatar/svelte";

    function initials(name: string): string {
        return name.slice(0, 2).toUpperCase();
    }

    let style = $state<AvatarFallbackStyle>("initials");
    let loading = $state(true);
    let saving = $state(false);
    let error = $state<string | null>(null);

    async function load() {
        if (!auth.token) return;
        loading = true;
        try {
            style = await getAvatarFallbackStyle(auth.token);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to load setting";
        } finally {
            loading = false;
        }
    }

    async function setStyle(next: AvatarFallbackStyle) {
        if (!auth.token || style === next) return;
        saving = true;
        try {
            await setAvatarFallbackStyle(auth.token, next);
            style = next;
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to save setting";
        } finally {
            saving = false;
        }
    }

    onMount(load);
</script>

{#if error}
    <div class="mb-4 px-3 py-2 rounded-md bg-destructive/10 border border-destructive/20 text-sm text-destructive">
        {error}
    </div>
{/if}

<div class="flex flex-col gap-6 max-w-2xl">
    <Card.Root>
        <Card.Header>
            <Card.Title>Avatar fallback</Card.Title>
            <Card.Description>
                How users without a profile image are shown across the app.
            </Card.Description>
        </Card.Header>
        <Card.Content>
            {#if loading}
                <div class="h-9 w-48 rounded bg-muted animate-pulse"></div>
            {:else}
                <div class="flex items-center justify-between gap-4">
                    <div class="flex items-center gap-2">
                        <Button
                            variant={style === "initials" ? "default" : "outline"}
                            size="sm"
                            onclick={() => setStyle("initials")}
                            disabled={saving}
                        >
                            Initials
                        </Button>
                        <Button
                            variant={style === "blobatar" ? "default" : "outline"}
                            size="sm"
                            onclick={() => setStyle("blobatar")}
                            disabled={saving}
                        >
                            Blobatar
                        </Button>
                    </div>
                    <div class="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-lg border {style === 'blobatar' ? '' : 'bg-muted'} text-xs font-medium">
                        {#if style === "blobatar"}
                            <Blobatar name={auth.user?.username ?? "user"} size={60} class="size-[150%]" />
                        {:else}
                            {initials(auth.user?.username ?? "??")}
                        {/if}
                    </div>
                </div>
            {/if}
        </Card.Content>
    </Card.Root>
</div>
