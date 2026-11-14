<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import ReleaseNotesDrawer from "$lib/components/shell/release-notes-drawer.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import { getReleases, getReleaseNotes, type ReleaseEntry, type ReleaseNotes, type ReleasesResponse } from "$lib/api/system";

    let releases = $state<ReleasesResponse | null>(null);
    let releasesLoading = $state(true);
    let error = $state<string | null>(null);
    let includePre = $state(true);
    let selected = $state<ReleaseEntry | null>(null);
    let notes = $state<ReleaseNotes | null>(null);
    let notesLoading = $state(false);

    async function loadReleases() {
        releasesLoading = true;
        try {
            releases = await getReleases(includePre);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to fetch releases";
        } finally {
            releasesLoading = false;
        }
    }

    async function openNotes(release: ReleaseEntry) {
        selected = release;
        notes = null;
        notesLoading = true;
        try {
            notes = await getReleaseNotes(release.version);
        } catch {
            notes = null;
        } finally {
            notesLoading = false;
        }
    }

    onMount(loadReleases);
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="Patch notes" description="What changed in each release.">
    <section class="cx-panel st-panel" aria-labelledby="pn-releases">
        <div class="st-split">
            <div>
                <h3 id="pn-releases">Releases</h3>
                <p>Open a release to read its patch notes.</p>
            </div>
            <label class="cx-check" for="include-pre">
                <Switch
                    checked={includePre}
                    label="Include pre-releases"
                    onchange={(value) => {
                        includePre = value;
                        loadReleases();
                    }}
                />
                Pre-releases
            </label>
        </div>
        {#if releasesLoading}
            <div class="db-skeleton-list" aria-hidden="true">
                {#each { length: 3 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
            </div>
        {:else}
            <div>
                {#each releases?.releases ?? [] as release (release.tag)}
                    <div class="st-release">
                        <div class="st-release__name">
                            {release.version}
                            {#if release.is_current}<span class="cx-badge cx-badge--success">Installed</span>{/if}
                            {#if release.prerelease}<span class="cx-badge">Pre-release</span>{/if}
                        </div>
                        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => openNotes(release)}>Patch notes</button>
                    </div>
                {/each}
            </div>
        {/if}
    </section>
</SettingsPage>

{#if selected}
    <ReleaseNotesDrawer open version={selected.version} prerelease={selected.prerelease} htmlUrl={selected.html_url} {notes} loading={notesLoading} onclose={() => (selected = null)} />
{/if}
