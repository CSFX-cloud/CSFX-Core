<script lang="ts">
    import SideDrawer from "$lib/components/shell/side-drawer.svelte";
    import type { NoteBlock, NoteCategory, ReleaseNotes } from "$lib/api/system";

    type Props = {
        open: boolean;
        version: string;
        prerelease: boolean;
        htmlUrl: string;
        notes: ReleaseNotes | null;
        loading?: boolean;
        onclose: () => void;
    };

    let { open, version, prerelease, htmlUrl, notes, loading = false, onclose }: Props = $props();

    const CATEGORIES: { key: NoteCategory; label: string; badge: string }[] = [
        { key: "new", label: "New", badge: "cx-badge--success" },
        { key: "improved", label: "Improved", badge: "" },
        { key: "fixed", label: "Fixed", badge: "cx-badge--warning" },
        { key: "security", label: "Security", badge: "cx-badge--danger" },
    ];

    const sections = $derived(
        CATEGORIES.map((category) => ({ ...category, blocks: notes?.sections[category.key] ?? [] })).filter((section) => section.blocks.length > 0),
    );

    function countEntries(blocks: NoteBlock[]): number {
        return blocks.reduce((total, block) => total + (block.type === "list" ? block.items.length : block.type === "heading" ? 1 : 0), 0);
    }
</script>

<SideDrawer {open} wide title={`v${version}`} subtitle={notes?.date} {onclose}>
    <div class="rn-meta">
        {#if prerelease}<span class="cx-badge cx-badge--warning">Pre-release</span>{:else}<span class="cx-badge">Stable</span>{/if}
    </div>

    {#if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: 4 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if notes}
        {#if notes.breaking.length > 0}
            <section class="rn-section rn-breaking" role="alert">
                <h3 class="rn-section__head">
                    <span class="cx-badge cx-badge--danger">Breaking changes</span>
                    <span class="cx-hint cx-mono">{notes.breaking.length}</span>
                </h3>
                <ul class="rn-list">
                    {#each notes.breaking as entry (entry)}<li>{entry}</li>{/each}
                </ul>
            </section>
        {/if}
        <div class="rn-sections">
            {#each sections as section (section.key)}
                <section class="rn-section">
                    <h3 class="rn-section__head">
                        <span class="cx-badge {section.badge}">{section.label}</span>
                        <span class="cx-hint cx-mono">{countEntries(section.blocks)}</span>
                    </h3>
                    <div class="rn-blocks">
                        {#each section.blocks as block, index (index)}
                            {#if block.type === "heading"}
                                <h4 class="rn-title">{block.text}</h4>
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
        </div>
    {:else}
        <p class="st-note">No patch notes are available for this release.</p>
    {/if}

    {#snippet footer()}
        <a class="cx-btn cx-btn--secondary cx-btn--sm" href={htmlUrl} target="_blank" rel="noreferrer">View on GitHub</a>
        <a class="cx-btn cx-btn--primary cx-btn--sm" href="/admin/settings">Update settings</a>
    {/snippet}
</SideDrawer>
