<script lang="ts">
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import {
        listResourceGroups,
        createResourceGroup,
        updateResourceGroup,
        suggestCidr,
        type ResourceGroup,
    } from "$lib/api/resource-groups";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import Icon from "@iconify/svelte";
    import IconPicker from "$lib/components/icon-picker.svelte";
    import RgIcon from "$lib/components/rg-icon.svelte";

    let groups = $state<ResourceGroup[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let creating = $state(false);
    let createDialog = $state<HTMLDialogElement | null>(null);
    let searchText = $state("");
    let pinnedScroller = $state<HTMLDivElement | null>(null);
    let viewMode = $state<"list" | "grid">(
        (typeof localStorage !== "undefined" && localStorage.getItem("rg-view-mode") as "list" | "grid") || "list",
    );

    function setViewMode(mode: "list" | "grid") {
        viewMode = mode;
        localStorage.setItem("rg-view-mode", mode);
    }

    function scrollPinned(direction: -1 | 1) {
        pinnedScroller?.scrollBy({ left: direction * 280, behavior: "smooth" });
    }

    let newName = $state("");
    let newCidr = $state("10.100.0.0/24");
    let newDescription = $state("");
    let newIcon = $state("mdi:cube-outline");
    let newColor = $state("#6366f1");
    let createError = $state<string | null>(null);

    async function load() {
        if (!auth.token) return;
        try {
            groups = await listResourceGroups(auth.token);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load resource groups";
        } finally {
            loading = false;
        }
    }

    async function openCreateDialog() {
        if (auth.token) {
            try {
                newCidr = await suggestCidr(auth.token);
            } catch {
                // keep default fallback below
            }
        }
        createDialog?.showModal();
    }

    async function handleCreate() {
        if (!auth.token || !newName || !newCidr) return;
        creating = true;
        createError = null;
        try {
            const created = await createResourceGroup(auth.token, {
                name: newName,
                description: newDescription || undefined,
                internal_cidr: newCidr,
                icon: newIcon,
                color: newColor,
            });
            groups = [...groups, created];
            createDialog?.close();
            newName = "";
            newCidr = "10.100.0.0/24";
            newDescription = "";
            newIcon = "mdi:cube-outline";
            newColor = "#6366f1";
        } catch (e) {
            createError = e instanceof Error ? e.message : "Failed to create resource group";
        } finally {
            creating = false;
        }
    }

    async function handleTogglePin(group: ResourceGroup) {
        if (!auth.token) return;
        try {
            const updated = await updateResourceGroup(auth.token, group.id, { pinned: !group.pinned });
            groups = groups.map((g) => (g.id === updated.id ? updated : g));
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to update resource group";
        }
    }

    function statusClass(status: string): string {
        switch (status.toLowerCase()) {
            case "active": return "text-green-500";
            case "deleting": return "text-red-500";
            case "suspended": return "text-yellow-500";
            default: return "text-muted-foreground";
        }
    }

    let pinnedGroups = $derived(groups.filter((g) => g.pinned));
    let filteredGroups = $derived(
        groups.filter((g) => !searchText || g.name.toLowerCase().includes(searchText.toLowerCase())),
    );

    let loadStarted = false;

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            load();
        }
    });
</script>

<dialog
    bind:this={createDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-md rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => { createError = null; }}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">New Resource Group</h2>
            <button
                class="text-muted-foreground hover:text-foreground"
                onclick={() => createDialog?.close()}
                aria-label="Close"
            >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
        </div>
        <div class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="rg-name">Name</label>
                <input
                    id="rg-name"
                    class="border rounded px-3 py-1.5 text-sm bg-background"
                    placeholder="production"
                    bind:value={newName}
                />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="rg-cidr">Internal CIDR</label>
                <input
                    id="rg-cidr"
                    class="border rounded px-3 py-1.5 text-sm bg-background font-mono"
                    placeholder="10.100.0.0/24"
                    bind:value={newCidr}
                />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="rg-desc">Description</label>
                <input
                    id="rg-desc"
                    class="border rounded px-3 py-1.5 text-sm bg-background"
                    placeholder="Optional"
                    bind:value={newDescription}
                />
            </div>
            <div class="flex flex-col gap-2 border-t pt-3">
                <IconPicker bind:icon={newIcon} bind:color={newColor} />
            </div>
        </div>
        {#if createError}
            <p class="text-xs text-destructive">{createError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => createDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleCreate} disabled={creating || !newName || !newCidr}>
                {creating ? "Creating..." : "Create"}
            </Button>
        </div>
    </div>
</dialog>

<header class="flex h-16 shrink-0 items-center gap-2 px-4 border-b">
    <Sidebar.Trigger class="-ms-1" />
    <span class="text-sm text-muted-foreground">/</span>
    <span class="text-sm font-medium">Resource Groups</span>
</header>

<div class="flex flex-col gap-6 p-6 min-w-0 overflow-x-hidden">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-xl font-semibold tracking-tight">Resource Groups</h1>
            <p class="text-sm text-muted-foreground mt-0.5">
                Isolated namespaces with dedicated internal networks
            </p>
        </div>
        <Button size="sm" onclick={openCreateDialog}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="12" y1="5" x2="12" y2="19"/>
                <line x1="5" y1="12" x2="19" y2="12"/>
            </svg>
            New Resource Group
        </Button>
    </div>

    {#if error}
        <p class="text-sm text-destructive">{error}</p>
    {/if}

    {#if pinnedGroups.length > 0}
        <div class="flex flex-col gap-2 min-w-0">
            <div
                bind:this={pinnedScroller}
                class="flex gap-3 overflow-x-auto min-w-0 scroll-smooth [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
            >
                {#each pinnedGroups as group (group.id)}
                    <button
                        class="flex items-center gap-3 shrink-0 w-64 border rounded-lg p-3 hover:bg-muted/30 transition-colors text-left"
                        onclick={() => goto(`/resource-groups/${group.id}`)}
                    >
                        <div
                            class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg overflow-hidden"
                            style="background-color: {group.color}20; color: {group.color};"
                        >
                            <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={18} />
                        </div>
                        <div class="min-w-0 flex-1">
                            <div class="flex items-center gap-1.5">
                                <p class="font-medium text-sm truncate">{group.name}</p>
                                <span class="text-xs px-1.5 py-0.5 rounded-full font-medium shrink-0 {statusClass(group.status)}">{group.status}</span>
                            </div>
                            <p class="text-xs text-muted-foreground font-mono truncate">{group.internal_cidr}</p>
                        </div>
                    </button>
                {/each}
            </div>
            <div class="flex items-center gap-1.5">
                <button
                    class="flex items-center justify-center w-6 h-6 rounded-full border text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    onclick={() => scrollPinned(-1)}
                    aria-label="Scroll left"
                    title="Scroll left"
                >
                    <Icon icon="mdi:chevron-left" width={14} height={14} />
                </button>
                <button
                    class="flex items-center justify-center w-6 h-6 rounded-full border text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    onclick={() => scrollPinned(1)}
                    aria-label="Scroll right"
                    title="Scroll right"
                >
                    <Icon icon="mdi:chevron-right" width={14} height={14} />
                </button>
            </div>
        </div>
    {/if}

    <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2 border rounded px-3 py-1.5 text-sm max-w-xs w-full">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-muted-foreground shrink-0"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
            <input class="bg-transparent outline-none flex-1 text-sm" placeholder="Search resource groups..." bind:value={searchText} />
        </div>
        <div class="flex items-center border rounded overflow-hidden shrink-0">
            <button
                class="flex items-center justify-center w-8 h-8 transition-colors {viewMode === 'list' ? 'bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => setViewMode("list")}
                aria-label="List view"
                title="List view"
            >
                <Icon icon="mdi:view-list" width={16} height={16} />
            </button>
            <button
                class="flex items-center justify-center w-8 h-8 transition-colors border-l {viewMode === 'grid' ? 'bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => setViewMode("grid")}
                aria-label="Grid view"
                title="Grid view"
            >
                <Icon icon="mdi:view-grid" width={16} height={16} />
            </button>
        </div>
    </div>

    {#if viewMode === "grid"}
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
            {#if loading}
                <p class="text-sm text-muted-foreground col-span-full text-center py-8">Loading...</p>
            {:else if filteredGroups.length === 0}
                <p class="text-sm text-muted-foreground col-span-full text-center py-8">
                    {groups.length === 0 ? "No resource groups. Create one to get started." : "No resource groups match search."}
                </p>
            {:else}
                {#each filteredGroups as group (group.id)}
                    <button
                        class="flex flex-col gap-3 border rounded-lg p-4 hover:bg-muted/30 transition-colors text-left"
                        onclick={() => goto(`/resource-groups/${group.id}`)}
                    >
                        <div class="flex items-center justify-between">
                            <div
                                class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg overflow-hidden"
                                style="background-color: {group.color}20; color: {group.color};"
                            >
                                <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={18} />
                            </div>
                            <span class="text-xs px-1.5 py-0.5 rounded-full font-medium {statusClass(group.status)}">{group.status}</span>
                        </div>
                        <div class="min-w-0">
                            <p class="font-medium text-sm truncate">{group.name}</p>
                            <p class="text-xs text-muted-foreground font-mono truncate mt-0.5">{group.internal_cidr}</p>
                            {#if group.description}
                                <p class="text-xs text-muted-foreground truncate mt-1">{group.description}</p>
                            {/if}
                        </div>
                        <p class="text-xs text-muted-foreground mt-auto">{group.created_at.slice(0, 10)}</p>
                    </button>
                {/each}
            {/if}
        </div>
    {:else}
    <div class="border rounded-lg overflow-hidden">
        <table class="w-full text-sm">
            <thead class="bg-muted/50">
                <tr>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Name</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">CIDR</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Description</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Status</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Created</th>
                </tr>
            </thead>
            <tbody>
                {#if loading}
                    <tr>
                        <td colspan="5" class="px-4 py-8 text-center text-muted-foreground">Loading...</td>
                    </tr>
                {:else if filteredGroups.length === 0}
                    <tr>
                        <td colspan="5" class="px-4 py-8 text-center text-muted-foreground">
                            {groups.length === 0 ? "No resource groups. Create one to get started." : "No resource groups match search."}
                        </td>
                    </tr>
                {:else}
                    {#each filteredGroups as group (group.id)}
                        <tr
                            class="border-t hover:bg-muted/30 transition-colors cursor-pointer"
                            onclick={() => goto(`/resource-groups/${group.id}`)}
                        >
                            <td class="px-4 py-3">
                                <div class="flex items-center gap-2.5">
                                    <div
                                        class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg overflow-hidden"
                                        style="background-color: {group.color}20; color: {group.color};"
                                    >
                                        <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={16} />
                                    </div>
                                    <span class="font-medium">{group.name}</span>
                                </div>
                            </td>
                            <td class="px-4 py-3 font-mono text-xs">{group.internal_cidr}</td>
                            <td class="px-4 py-3 text-muted-foreground">{group.description ?? "-"}</td>
                            <td class="px-4 py-3">
                                <span class="font-medium {statusClass(group.status)}">{group.status}</span>
                            </td>
                            <td class="px-4 py-3 text-xs text-muted-foreground">{group.created_at.slice(0, 10)}</td>
                        </tr>
                    {/each}
                {/if}
            </tbody>
        </table>
    </div>
    {/if}
</div>
