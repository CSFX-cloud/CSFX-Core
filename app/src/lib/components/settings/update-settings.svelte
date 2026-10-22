<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import {
        getUpdateStatus,
        getReleases,
        triggerUpdate,
        pauseUpdate,
        resumeUpdate,
        type UpdateStatus,
        type ReleasesResponse,
    } from "$lib/api/system";

    let status = $state<UpdateStatus | null>(null);
    let releases = $state<ReleasesResponse | null>(null);
    let statusLoading = $state(true);
    let releasesLoading = $state(true);
    let error = $state<string | null>(null);
    let actionLoading = $state(false);
    let includePre = $state(false);
    let pollInterval: ReturnType<typeof setInterval> | null = null;

    async function fetchStatus() {
        if (!auth.token) return;
        try {
            status = await getUpdateStatus(auth.token);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to fetch update status";
        } finally {
            statusLoading = false;
        }
    }

    async function fetchReleases() {
        if (!auth.token) return;
        releasesLoading = true;
        try {
            releases = await getReleases(auth.token, includePre);
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to fetch releases";
        } finally {
            releasesLoading = false;
        }
    }

    onMount(() => {
        fetchStatus();
        fetchReleases();
        pollInterval = setInterval(fetchStatus, 5000);
    });

    onDestroy(() => {
        if (pollInterval) clearInterval(pollInterval);
    });

    async function handleTrigger(version: string) {
        if (!auth.token) return;
        actionLoading = true;
        try {
            await triggerUpdate(auth.token, version);
            await fetchStatus();
        } catch (e) {
            error = e instanceof Error ? e.message : "trigger update failed";
        } finally {
            actionLoading = false;
        }
    }

    async function handlePause() {
        if (!auth.token) return;
        actionLoading = true;
        try {
            await pauseUpdate(auth.token);
            await fetchStatus();
        } catch (e) {
            error = e instanceof Error ? e.message : "pause failed";
        } finally {
            actionLoading = false;
        }
    }

    async function handleResume() {
        if (!auth.token) return;
        actionLoading = true;
        try {
            await resumeUpdate(auth.token);
            await fetchStatus();
        } catch (e) {
            error = e instanceof Error ? e.message : "resume failed";
        } finally {
            actionLoading = false;
        }
    }

    function buildStatusLabel(s: string | null): string {
        if (!s) return "idle";
        return s.replace(/_/g, " ").toLowerCase();
    }

    function buildStatusColor(s: string | null): string {
        if (!s) return "text-muted-foreground";
        const lower = s.toLowerCase();
        if (lower.includes("success") || lower.includes("done")) return "text-green-500";
        if (lower.includes("fail") || lower.includes("error")) return "text-red-500";
        if (lower.includes("running") || lower.includes("pending")) return "text-yellow-500";
        return "text-muted-foreground";
    }

    function isActive(s: string | null): boolean {
        if (!s) return false;
        const lower = s.toLowerCase();
        return lower.includes("running") || lower.includes("pending") || lower.includes("building");
    }
</script>

{#snippet kv(label: string, value: string, tone: string)}
    <div class="cx-stat">
        <p class="cx-stat__label">{label}</p>
        <p class="db-stat-value cx-mono" style="font-size: 14px; color: {tone}">{value}</p>
    </div>
{/snippet}

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="Update" description="The installed version and the releases you can move to.">
    <section class="cx-panel st-panel" aria-labelledby="st-version">
        <div class="st-split">
            <div>
                <h3 id="st-version">System version</h3>
                <p>Current installation and update state.</p>
            </div>
            {#if status}<span class="cx-badge {status.paused ? 'cx-badge--warning' : 'cx-badge--success'}">{status.paused ? "Paused" : "Active"}</span>{/if}
        </div>
        {#if statusLoading}
            <div class="db-skeleton db-skeleton--row" aria-hidden="true"></div>
        {:else if status}
            <div class="db-stats">
                {@render kv("Installed", status.current_version, "var(--ink)")}
                {#if status.desired_version && status.desired_version !== status.current_version}
                    {@render kv("Scheduled", status.desired_version, "var(--warning)")}
                {:else}
                    {@render kv("Target", "up to date", "var(--ink-muted)")}
                {/if}
                {#if status.build_status}
                    {@render kv("Build status", buildStatusLabel(status.build_status), isActive(status.build_status) ? "var(--warning)" : "var(--ink)")}
                {/if}
                {#if status.last_result}
                    {@render kv("Last result", buildStatusLabel(status.last_result), "var(--ink)")}
                {/if}
            </div>
            <div class="st-split">
                <p class="cx-hint" style="flex: 1">
                    {status.paused ? "Updates are paused. No automatic upgrades will run." : "Updates are enabled. The system checks for new releases automatically."}
                </p>
                {#if !status.paused}
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionLoading} onclick={handlePause}>Pause updates</button>
                {:else}
                    <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={actionLoading} onclick={handleResume}>Resume updates</button>
                {/if}
            </div>
        {/if}
    </section>

    <section class="cx-panel st-panel" aria-labelledby="st-releases">
        <div class="st-split">
            <div>
                <h3 id="st-releases">Available releases</h3>
                <p>Select a version to update or downgrade.</p>
            </div>
            <label class="cx-check" for="include-pre">
                <Switch
                    checked={includePre}
                    label="Include pre-releases"
                    onchange={(value) => {
                        includePre = value;
                        fetchReleases();
                    }}
                />
                Pre-releases
            </label>
        </div>
        {#if releasesLoading}
            <div class="db-skeleton-list" aria-hidden="true">
                {#each { length: 3 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
            </div>
        {:else if releases && releases.releases.length > 0}
            <div>
                {#each releases.releases as release (release.tag)}
                    <div class="st-release">
                        <div style="min-width: 0">
                            <div class="st-release__name">
                                {release.version}
                                {#if release.is_current}<span class="cx-badge cx-badge--success">Installed</span>
                                {:else if release.is_newer}<span class="cx-badge cx-badge--warning">Newer</span>{/if}
                                {#if release.prerelease}<span class="cx-badge">Pre-release</span>{/if}
                            </div>
                            {#if release.name}<p class="cx-hint" style="margin-top: 2px">{release.name}</p>{/if}
                        </div>
                        {#if release.is_current}
                            <span class="cx-hint">Current</span>
                        {:else}
                            <button class="cx-btn {release.is_newer ? 'cx-btn--primary' : 'cx-btn--secondary'} cx-btn--sm" type="button" disabled={actionLoading} onclick={() => handleTrigger(release.version)}>
                                {release.is_newer ? "Update" : "Downgrade"}
                            </button>
                        {/if}
                    </div>
                {/each}
            </div>
        {:else}
            <p class="cx-hint">No releases found</p>
        {/if}
    </section>
</SettingsPage>
