<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getLogsRetention, updateLogsRetention } from "$lib/api/logs";

    const MIN_RETENTION_DAYS = 1;
    const MAX_RETENTION_DAYS = 365;

    let retentionDays = $state<number | null>(null);
    let inputValue = $state("");
    let loading = $state(true);
    let saving = $state(false);
    let error = $state<string | null>(null);
    let saved = $state(false);

    async function load() {
        if (!auth.token) return;
        loading = true;
        try {
            const response = await getLogsRetention(auth.token);
            retentionDays = response.retention_days;
            inputValue = String(response.retention_days);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to load retention setting";
        } finally {
            loading = false;
        }
    }

    async function save() {
        if (!auth.token) return;
        const days = Number(inputValue);
        if (!Number.isInteger(days) || days < MIN_RETENTION_DAYS || days > MAX_RETENTION_DAYS) {
            error = `retention days must be between ${MIN_RETENTION_DAYS} and ${MAX_RETENTION_DAYS}`;
            return;
        }
        saving = true;
        saved = false;
        try {
            const response = await updateLogsRetention(auth.token, days);
            retentionDays = response.retention_days;
            error = null;
            saved = true;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to save retention setting";
        } finally {
            saving = false;
        }
    }

    onMount(load);
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="Logs" description="How long events are kept on this cluster.">
    <section class="cx-panel st-panel" aria-labelledby="st-retention">
        <div>
            <h3 id="st-retention">Log retention</h3>
            <p>Logs older than this many days are deleted automatically every hour.</p>
        </div>
        {#if loading}
            <div class="db-skeleton db-skeleton--row" style="width: 200px" aria-hidden="true"></div>
        {:else}
            <form
                class="cx-field"
                style="max-width: 280px"
                onsubmit={(event) => {
                    event.preventDefault();
                    save();
                }}
            >
                <label class="cx-label" for="retention-input">Retention (days)</label>
                <div class="st-row">
                    <input class="cx-input" id="retention-input" type="number" min={MIN_RETENTION_DAYS} max={MAX_RETENTION_DAYS} bind:value={inputValue} />
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="submit" style="height: 44px" disabled={saving || inputValue === String(retentionDays)} aria-busy={saving}>
                        {saving ? "Saving" : "Save"}
                    </button>
                </div>
                {#if saved && !saving}<p class="cx-hint" style="color: var(--success)">Saved</p>{/if}
            </form>
        {/if}
    </section>
</SettingsPage>
