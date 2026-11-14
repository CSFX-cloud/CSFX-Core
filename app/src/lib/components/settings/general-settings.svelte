<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getSshAccess, setSshAccess, type SshAccess } from "$lib/auth/api";
    import { theme, type ThemePreference } from "$lib/utils/theme.svelte";

    const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
        { value: "system", label: "System" },
        { value: "light", label: "Light" },
        { value: "dark", label: "Dark" },
    ];

    let error = $state<string | null>(null);

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
        try {
            ssh = await getSshAccess();
        } catch {
            ssh = null;
        }
    }

    async function updateSsh(enabled: boolean) {
        sshSaving = true;
        try {
            ssh = await setSshAccess(enabled, sshHours);
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

    onMount(loadSsh);
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="General" description="Cluster-wide preferences.">
    <section class="cx-panel st-panel" aria-labelledby="st-look">
        <div class="st-toggle">
            <div>
                <h3 id="st-look">Appearance</h3>
                <p>System follows your device setting. Applies to this browser only.</p>
            </div>
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Theme">
                {#each THEME_OPTIONS as option (option.value)}
                    <button class="cx-tab" role="tab" type="button" aria-selected={theme.preference === option.value} onclick={() => theme.set(option.value)}>{option.label}</button>
                {/each}
            </div>
        </div>
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
