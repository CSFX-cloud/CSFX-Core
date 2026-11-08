<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import { CircleArrowUp } from "@lucide/svelte";
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

    const scheduled = $derived(!!status?.desired_version && status.desired_version !== status.current_version);
    const targetVersion = $derived(scheduled ? status?.desired_version : (releases?.latest_update ?? null));
    const isPrerelease = $derived(!!status?.current_version.includes("-"));
    const heroTitle = $derived(status?.paused ? "Updates paused" : scheduled ? "Update scheduled" : releases?.latest_update ? "Update available" : "Up to date");
    const heroBadge = $derived(
        status?.paused
            ? { label: "Paused", tone: "cx-badge--warning" }
            : scheduled || releases?.latest_update
              ? { label: "Newer", tone: "cx-badge--warning" }
              : { label: "Active", tone: "cx-badge--success" },
    );

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

<SettingsPage title="Update" description="New csfx.cloud versions roll out as releases. You can pause updates at any point.">
    {#if statusLoading}
        <div class="db-skeleton db-skeleton--row" aria-hidden="true"></div>
    {:else if status}
        <section class="cx-stage" aria-label="Update status">
            <div class="cx-stage__canvas"></div>
            <div class="rd-head">
                <div class="rd-id" style="align-items: flex-start">
                    <span class="up-tile"><CircleArrowUp class="cx-i" aria-hidden="true" /></span>
                    <div style="min-width: 0">
                        <div class="rd-title">
                            <h1>{heroTitle}</h1>
                            <span class="cx-badge {heroBadge.tone}">{heroBadge.label}</span>
                        </div>
                        <p class="up-sub">
                            {status.paused ? "Updates are paused. No automatic upgrades will run." : "Updates are enabled. The system checks for new releases automatically."}
                        </p>
                        <div class="up-vers">
                            <span>Installed <b>{status.current_version}</b></span>
                            <span class="rs-arrow">to</span>
                            <span>Target <b>{targetVersion ?? "up to date"}</b></span>
                            <span class="rs-chip">channel: {isPrerelease ? "pre-release" : "stable"}</span>
                        </div>
                    </div>
                </div>
                <div class="db-actions">
                    <a class="cx-btn cx-btn--secondary cx-btn--sm" href="/admin/settings/patch-notes">Patch notes</a>
                    {#if releases?.latest_update && !scheduled}
                        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={actionLoading} onclick={() => releases?.latest_update && handleTrigger(releases.latest_update)}>
                            Update to {releases.latest_update}
                        </button>
                    {/if}
                    {#if !status.paused}
                        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={actionLoading} onclick={handlePause}>Pause updates</button>
                    {:else}
                        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={actionLoading} onclick={handleResume}>Resume updates</button>
                    {/if}
                </div>
            </div>
        </section>
        {#if status.build_status || status.last_result}
            <div class="db-stats">
                {#if status.build_status}
                    {@render kv("Build status", buildStatusLabel(status.build_status), isActive(status.build_status) ? "var(--warning)" : "var(--ink)")}
                {/if}
                {#if status.last_result}
                    {@render kv("Last result", buildStatusLabel(status.last_result), "var(--ink)")}
                {/if}
            </div>
        {/if}
    {/if}

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
