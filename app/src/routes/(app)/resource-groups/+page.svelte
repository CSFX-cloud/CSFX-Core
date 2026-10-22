<script lang="ts">
    import { untrack } from "svelte";
    import { Layers, LayoutGrid, List, Plus, Search } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { listResourceGroups, type ResourceGroup } from "$lib/api/resource-groups";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import CreateGroupDialog from "$lib/components/resource-group/create-group-dialog.svelte";
    import GroupsGrid from "$lib/components/resource-group/groups-grid.svelte";
    import GroupsTable from "$lib/components/resource-group/groups-table.svelte";

    type ViewMode = "list" | "grid";

    const VIEW_STORAGE_KEY = "rg-view-mode";
    const SKELETON_ROWS = 3;

    let groups = $state<ResourceGroup[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let query = $state("");
    let createDialog = $state<ReturnType<typeof CreateGroupDialog> | null>(null);
    let view = $state<ViewMode>(storedView());

    const visible = $derived(
        groups
            .filter((group) => group.name.toLowerCase().includes(query.trim().toLowerCase()))
            .sort((a, b) => Number(b.pinned) - Number(a.pinned) || a.name.localeCompare(b.name)),
    );

    function storedView(): ViewMode {
        const stored = typeof localStorage === "undefined" ? null : localStorage.getItem(VIEW_STORAGE_KEY);
        return stored === "grid" ? "grid" : "list";
    }

    function setView(mode: ViewMode) {
        view = mode;
        localStorage.setItem(VIEW_STORAGE_KEY, mode);
    }

    async function load() {
        if (!auth.token) return;
        try {
            groups = await listResourceGroups(auth.token);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load resource groups";
        } finally {
            loading = false;
        }
    }

    $effect(() => {
        if (auth.token) untrack(load);
    });
</script>

<div class="db-content">
    <section class="cx-stage db-hero" aria-label="Resource groups">
        <div class="cx-stage__canvas"></div>
        <div class="db-hero__in">
            <h1>Resource groups.<br /><span style="color: var(--ink-muted)">Isolated namespaces, each with its own network.</span></h1>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => createDialog?.open()}>
                <Plus class="cx-i" aria-hidden="true" />
                New resource group
            </button>
        </div>
    </section>

    <div class="db-toolbar">
        <div class="cx-input-wrap db-search">
            <Search class="cx-i" aria-hidden="true" />
            <input class="cx-input cx-input--icon" type="search" placeholder="Search resource groups" aria-label="Search resource groups" bind:value={query} />
        </div>
        <div class="rg-view" role="group" aria-label="View">
            <button type="button" aria-label="List view" aria-pressed={view === "list"} onclick={() => setView("list")}>
                <List class="cx-i" aria-hidden="true" />
            </button>
            <button type="button" aria-label="Grid view" aria-pressed={view === "grid"} onclick={() => setView("grid")}>
                <LayoutGrid class="cx-i" aria-hidden="true" />
            </button>
        </div>
    </div>

    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if groups.length === 0}
        <EmptyState icon={Layers} label="No resource groups yet" />
    {:else if visible.length === 0}
        <EmptyState icon={Search} label="No resource groups match the search" />
    {:else if view === "grid"}
        <GroupsGrid groups={visible} oncreate={() => createDialog?.open()} />
    {:else}
        <GroupsTable groups={visible} />
    {/if}
</div>

<CreateGroupDialog bind:this={createDialog} oncreated={(group) => (groups = [...groups, group])} />
