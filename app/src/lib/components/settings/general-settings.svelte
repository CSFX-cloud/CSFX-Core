<script lang="ts">
    import { onMount } from "svelte";
    import * as Card from "$lib/components/ui/card/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { auth } from "$lib/auth/store.svelte";
    import {
        getAvatarFallbackStyle,
        getSshAccess,
        setAvatarFallbackStyle,
        setSshAccess,
        type AvatarFallbackStyle,
        type SshAccess,
    } from "$lib/auth/api";
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

    const SSH_DURATIONS: { label: string; hours: number | null }[] = [
        { label: "1 hour", hours: 1 },
        { label: "8 hours", hours: 8 },
        { label: "24 hours", hours: 24 },
        { label: "Until disabled", hours: null },
    ];

    let ssh = $state<SshAccess | null>(null);
    let sshSaving = $state(false);
    let sshHours = $state<number | null>(8);

    async function loadSsh() {
        if (!auth.token) return;
        try {
            ssh = await getSshAccess(auth.token);
        } catch {
            ssh = null;
        }
    }

    async function updateSsh(enabled: boolean) {
        if (!auth.token) return;
        sshSaving = true;
        try {
            ssh = await setSshAccess(auth.token, enabled, sshHours);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to save setting";
        } finally {
            sshSaving = false;
        }
    }

    function describeExpiry(access: SshAccess): string {
        if (!access.expires_at) return "Open until you disable it.";
        return `Open until ${new Date(access.expires_at).toLocaleString()}.`;
    }

    onMount(() => {
        load();
        loadSsh();
    });
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

    {#if ssh}
        <Card.Root>
            <Card.Header>
                <Card.Title>SSH access</Card.Title>
                <Card.Description>
                    Closed by default. When open, administrators with a registered public key can
                    sign in as user csfx on every node. Passwords are never accepted.
                </Card.Description>
            </Card.Header>
            <Card.Content class="flex flex-col gap-4">
                {#if ssh.effective}
                    <div class="flex items-center justify-between gap-4">
                        <p class="text-sm">{describeExpiry(ssh)}</p>
                        <Button variant="outline" size="sm" disabled={sshSaving} onclick={() => updateSsh(false)}>
                            Disable
                        </Button>
                    </div>
                {:else}
                    <div class="flex flex-wrap items-center gap-2">
                        {#each SSH_DURATIONS as option (option.label)}
                            <Button
                                variant={sshHours === option.hours ? "default" : "outline"}
                                size="sm"
                                onclick={() => (sshHours = option.hours)}
                            >
                                {option.label}
                            </Button>
                        {/each}
                    </div>
                    <div>
                        <Button size="sm" disabled={sshSaving} onclick={() => updateSsh(true)}>
                            Enable SSH access
                        </Button>
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>
    {/if}
</div>
