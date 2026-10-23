<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getNtfySettings, sendTestNotification, updateNtfySettings, type NtfySeverity } from "$lib/api/notifications";

    const SEVERITIES: { value: NtfySeverity; label: string }[] = [
        { value: "info", label: "Info and above" },
        { value: "warning", label: "Warning and above" },
        { value: "critical", label: "Critical only" },
    ];

    let enabled = $state(false);
    let serverUrl = $state("");
    let topic = $state("");
    let accessToken = $state("");
    let tokenSet = $state(false);
    let minSeverity = $state<NtfySeverity>("warning");
    let loading = $state(true);
    let saving = $state(false);
    let testing = $state(false);
    let error = $state<string | null>(null);
    let notice = $state<string | null>(null);

    async function load() {
        if (!auth.token) return;
        try {
            const settings = await getNtfySettings(auth.token);
            enabled = settings.enabled;
            serverUrl = settings.server_url;
            topic = settings.topic;
            tokenSet = settings.token_set;
            minSeverity = settings.min_severity;
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to load notification settings";
        } finally {
            loading = false;
        }
    }

    async function save() {
        if (!auth.token) return;
        saving = true;
        notice = null;
        try {
            const settings = await updateNtfySettings(auth.token, {
                enabled,
                server_url: serverUrl,
                topic,
                token: accessToken === "" ? undefined : accessToken,
                min_severity: minSeverity,
            });
            tokenSet = settings.token_set;
            accessToken = "";
            error = null;
            notice = "Saved";
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to save notification settings";
        } finally {
            saving = false;
        }
    }

    async function sendTest() {
        if (!auth.token) return;
        testing = true;
        notice = null;
        try {
            await sendTestNotification(auth.token);
            error = null;
            notice = "Test notification sent";
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to send test notification";
        } finally {
            testing = false;
        }
    }

    async function clearToken() {
        if (!auth.token) return;
        accessToken = "";
        saving = true;
        try {
            const settings = await updateNtfySettings(auth.token, { enabled, server_url: serverUrl, topic, token: "", min_severity: minSeverity });
            tokenSet = settings.token_set;
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to clear access token";
        } finally {
            saving = false;
        }
    }

    onMount(load);
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="Notifications" description="Publish cluster alerts such as high container load or node failures to ntfy.">
    <section class="cx-panel st-panel" aria-labelledby="st-ntfy">
        <div>
            <h3 id="st-ntfy">ntfy</h3>
            <p>Every new alert is published to the topic below when it reaches the minimum severity.</p>
        </div>
        {#if loading}
            <div class="db-skeleton db-skeleton--row" style="width: 280px" aria-hidden="true"></div>
        {:else}
            <form
                class="st-panel"
                style="padding: 0"
                onsubmit={(event) => {
                    event.preventDefault();
                    save();
                }}
            >
                <div class="st-row" style="align-items: center">
                    <Switch checked={enabled} label="Publish alerts to ntfy" onchange={(value) => (enabled = value)} />
                    <span>Publish alerts to ntfy</span>
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="ntfy-server">Server URL</label>
                    <input class="cx-input" id="ntfy-server" type="url" placeholder="https://ntfy.sh" bind:value={serverUrl} required />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="ntfy-topic">Topic</label>
                    <input class="cx-input" id="ntfy-topic" type="text" autocomplete="off" bind:value={topic} required />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="ntfy-token">Access token</label>
                    <div class="st-row">
                        <input class="cx-input" id="ntfy-token" type="password" autocomplete="off" placeholder={tokenSet ? "Token is set" : "Optional"} bind:value={accessToken} />
                        {#if tokenSet}
                            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" style="height: 44px" disabled={saving} onclick={clearToken}>Remove</button>
                        {/if}
                    </div>
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="ntfy-severity">Minimum severity</label>
                    <select class="cx-input" id="ntfy-severity" bind:value={minSeverity}>
                        {#each SEVERITIES as option (option.value)}
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                </div>
                <div class="db-actions">
                    <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving} aria-busy={saving}>{saving ? "Saving" : "Save"}</button>
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={testing || saving} aria-busy={testing} onclick={sendTest}>
                        {testing ? "Sending" : "Send test"}
                    </button>
                </div>
                {#if notice}<p class="cx-hint" style="color: var(--success)">{notice}</p>{/if}
            </form>
        {/if}
    </section>
</SettingsPage>
