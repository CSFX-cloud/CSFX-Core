<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
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

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="General" description="Cluster-wide preferences.">
    <section class="cx-panel st-panel" aria-labelledby="st-avatar">
        <div class="st-split">
            <div>
                <h3 id="st-avatar">Avatar fallback</h3>
                <p>How users without a profile image are shown across the app.</p>
            </div>
            {#if !loading}
                <span class="st-avatar" style="width: 44px; height: 44px; font-size: 13px" aria-hidden="true">
                    {#if style === "blobatar"}
                        <Blobatar name={auth.user?.username ?? "user"} size={60} class="size-[150%]" />
                    {:else}
                        {initials(auth.user?.username ?? "??")}
                    {/if}
                </span>
            {/if}
        </div>
        {#if loading}
            <div class="db-skeleton db-skeleton--row" style="width: 200px" aria-hidden="true"></div>
        {:else}
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Avatar fallback" style="justify-self: start">
                <button class="cx-tab" role="tab" type="button" aria-selected={style === "initials"} disabled={saving} onclick={() => setStyle("initials")}>Initials</button>
                <button class="cx-tab" role="tab" type="button" aria-selected={style === "blobatar"} disabled={saving} onclick={() => setStyle("blobatar")}>Blobatar</button>
            </div>
        {/if}
    </section>

    {#if ssh}
        <section class="cx-panel st-panel" aria-labelledby="st-ssh">
            <div>
                <h3 id="st-ssh">SSH access</h3>
                <p>
                    Closed by default. When open, administrators with a registered public key can sign in as user csfx on every node.
                    Passwords are never accepted.
                </p>
            </div>
            {#if ssh.effective}
                <div class="st-split">
                    <p class="st-note" style="flex: 1">{describeExpiry(ssh)}</p>
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={sshSaving} onclick={() => updateSsh(false)}>Disable</button>
                </div>
            {:else}
                <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Duration" style="justify-self: start; flex-wrap: wrap">
                    {#each SSH_DURATIONS as option (option.label)}
                        <button class="cx-tab" role="tab" type="button" aria-selected={sshHours === option.hours} onclick={() => (sshHours = option.hours)}>{option.label}</button>
                    {/each}
                </div>
                <div>
                    <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={sshSaving} aria-busy={sshSaving} onclick={() => updateSsh(true)}>Enable SSH access</button>
                </div>
            {/if}
        </section>
    {/if}
</SettingsPage>
