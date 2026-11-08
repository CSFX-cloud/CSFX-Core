<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getReleases, getReleaseNotes, type NoteCategory, type ReleaseEntry, type ReleaseNotes, type ReleasesResponse } from "$lib/api/system";

    const CATEGORIES: { key: NoteCategory; label: string }[] = [
        { key: "new", label: "New" },
        { key: "improved", label: "Improved" },
        { key: "fixed", label: "Fixed" },
        { key: "security", label: "Security" },
    ];

    let releases = $state<ReleasesResponse | null>(null);
    let selected = $state<ReleaseEntry | null>(null);
    let notes = $state<ReleaseNotes | null>(null);
    let releasesLoading = $state(true);
    let notesLoading = $state(false);
    let error = $state<string | null>(null);
    let includePre = $state(true);

    const sections = $derived(CATEGORIES.map((category) => ({ ...category, blocks: notes?.sections[category.key] ?? [] })).filter((section) => section.blocks.length > 0));

    async function select(release: ReleaseEntry) {
        if (!auth.token) return;
        selected = release;
        notesLoading = true;
        try {
            notes = await getReleaseNotes(auth.token, release.version);
            error = null;
        } catch (e) {
            notes = null;
            error = e instanceof Error ? e.message : "failed to fetch patch notes";
        } finally {
            notesLoading = false;
        }
    }

    async function loadReleases() {
        if (!auth.token) return;
        releasesLoading = true;
        try {
            releases = await getReleases(auth.token, includePre);
            const first = releases.releases.find((release) => release.version === releases?.latest_update) ?? releases.releases[0];
            if (first) await select(first);
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to fetch releases";
        } finally {
            releasesLoading = false;
        }
    }

    onMount(loadReleases);
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="Patch notes" description="What changed in each release.">
    <div class="pn-cols">
        <section class="cx-panel st-panel" aria-labelledby="pn-notes">
            <div class="st-split">
                <div>
                    <h3 id="pn-notes">{selected ? `What is new in v${selected.version}` : "Patch notes"}</h3>
                    {#if notes}<p>Released {notes.date}</p>{/if}
                </div>
                {#if selected}
                    <span class="cx-badge {selected.prerelease ? 'cx-badge--warning' : ''}">{selected.prerelease ? "Pre-release" : "Stable"}</span>
                {/if}
            </div>

            {#if releasesLoading || notesLoading}
                <div class="db-skeleton-list" aria-hidden="true">
                    {#each { length: 4 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
                </div>
            {:else if notes}
                {#if notes.breaking.length > 0}
                    <section class="rn-section rn-breaking" role="alert">
                        <h4 class="pn-head">Breaking changes</h4>
                        <ul class="rn-list">
                            {#each notes.breaking as entry (entry)}<li>{entry}</li>{/each}
                        </ul>
                    </section>
                {/if}
                {#each sections as section (section.key)}
                    <section class="rn-section">
                        <h4 class="pn-head">{section.label}</h4>
                        <div class="rn-blocks">
                            {#each section.blocks as block, index (index)}
                                {#if block.type === "heading"}
                                    <h5 class="rn-title">{block.text}</h5>
                                {:else if block.type === "paragraph"}
                                    <p class="rn-text">{block.text}</p>
                                {:else if block.type === "list"}
                                    <ul class="rn-list">
                                        {#each block.items as item (item)}<li>{item}</li>{/each}
                                    </ul>
                                {:else}
                                    <figure class="rn-figure">
                                        <img src={block.src} alt={block.alt} loading="lazy" />
                                        <figcaption class="cx-hint">{block.alt}</figcaption>
                                    </figure>
                                {/if}
                            {/each}
                        </div>
                    </section>
                {/each}
            {:else if selected}
                <p class="cx-hint">No patch notes are available for this release.</p>
                <div><a class="cx-btn cx-btn--secondary cx-btn--sm" href={selected.html_url} target="_blank" rel="noreferrer">View on GitHub</a></div>
            {/if}
        </section>

        <section class="cx-panel st-panel" aria-labelledby="pn-releases">
            <div class="st-split">
                <div>
                    <h3 id="pn-releases">Releases</h3>
                    <p>Pick a version to read.</p>
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
                    Pre
                </label>
            </div>
            <div>
                {#each releases?.releases ?? [] as release (release.tag)}
                    <div class="st-release">
                        <div class="st-release__name">
                            {release.version}
                            {#if release.is_current}<span class="cx-badge cx-badge--success">Installed</span>{/if}
                        </div>
                        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" aria-pressed={selected?.tag === release.tag} disabled={selected?.tag === release.tag} onclick={() => select(release)}>
                            {selected?.tag === release.tag ? "Shown" : "Show"}
                        </button>
                    </div>
                {/each}
            </div>
        </section>
    </div>
</SettingsPage>
