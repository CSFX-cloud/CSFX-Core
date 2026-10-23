<script lang="ts">
    import { tick } from "svelte";
    import { goto } from "$app/navigation";
    import { Bell, Box, Layers, Monitor, Search, Server, SunMoon, TriangleAlert, Database } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { commandPalette } from "./command-palette-store.svelte.js";
    import { highlight, search, suggestions, type PaletteItem } from "./command-palette-search";
    import { listNodes, type Node } from "$lib/api/nodes";
    import { listWorkloads, listResourceGroups, listBuckets, type Workload, type ResourceGroup, type Bucket } from "$lib/api/resource-groups";
    import { listEvents, type AlertEvent } from "$lib/api/events";
    import { NOTIFICATIONS_NAV, PRIMARY_NAV, RESOURCE_NAV, SETTINGS_NAV } from "$lib/components/shell/navigation";
    import { theme } from "$lib/utils/theme.svelte";

    const TILE_COUNT = 6;

    let query = $state("");
    let selected = $state(-1);
    let loading = $state(false);
    let input = $state<HTMLInputElement>();
    let list = $state<HTMLElement>();
    let nodes = $state<Node[]>([]);
    let workloads = $state<Workload[]>([]);
    let resourceGroups = $state<ResourceGroup[]>([]);
    let buckets = $state<Bucket[]>([]);
    let alerts = $state<AlertEvent[]>([]);

    async function loadData() {
        if (!auth.token || loading) return;
        loading = true;
        try {
            [nodes, workloads, resourceGroups, buckets, alerts] = await Promise.all([
                listNodes(auth.token),
                listWorkloads(auth.token),
                listResourceGroups(auth.token),
                listBuckets(auth.token),
                listEvents(auth.token, { status: "open" }),
            ]);
        } catch {
            return;
        } finally {
            loading = false;
        }
    }

    const groupName = (id: string | null) => resourceGroups.find((group) => group.id === id)?.name ?? "None";
    const workloadHref = (w: Workload) => (w.runtime_class === "vm" && w.resource_group_id ? `/resource-groups/${w.resource_group_id}/vm/${w.id}` : `/resource-groups/${w.resource_group_id}`);

    const pageItems = $derived<PaletteItem[]>(
        [...PRIMARY_NAV, ...RESOURCE_NAV, NOTIFICATIONS_NAV, SETTINGS_NAV].map((page) => ({
            id: `page-${page.href}`,
            group: "Pages",
            title: page.title,
            subtitle: page.href,
            kind: "Page",
            icon: page.icon,
            keywords: "",
            meta: [["Opens", page.href]],
            cta: "Open",
            href: page.href,
        })),
    );

    const actionItems = $derived<PaletteItem[]>([
        {
            id: "action-theme",
            group: "Actions",
            title: "Switch appearance",
            subtitle: "Light / Dark",
            kind: "Action",
            icon: SunMoon,
            keywords: "theme dark light mode",
            meta: [["Current", theme.resolved]],
            cta: "Switch",
            href: "#",
            action: () => theme.set(theme.resolved === "dark" ? "light" : "dark"),
        },
        {
            id: "action-ntfy",
            group: "Actions",
            title: "Notification settings",
            subtitle: "ntfy",
            kind: "Action",
            icon: Bell,
            keywords: "alerts push publish topic",
            meta: [["Section", "Settings"]],
            cta: "Open",
            href: "/admin/settings/notifications",
        },
    ]);

    const resourceItems = $derived<PaletteItem[]>([
        ...nodes.map((n) => ({
            id: `node-${n.id}`,
            group: "Nodes",
            title: n.hostname,
            subtitle: n.ip_address ?? "",
            kind: `Node · ${n.role}`,
            icon: Server,
            keywords: n.name,
            meta: [["Status", n.status], ["Version", n.agent_version]] as [string, string][],
            cta: "Open node",
            href: `/nodes/${n.id}`,
        })),
        ...workloads.map((w) => ({
            id: `workload-${w.id}`,
            group: "Workloads",
            title: w.name,
            subtitle: w.image,
            kind: w.runtime_class === "vm" ? "Virtual machine" : "Container",
            icon: w.runtime_class === "vm" ? Monitor : Box,
            keywords: "",
            meta: [["Group", groupName(w.resource_group_id)], ["Image", w.image], ["Status", w.status]] as [string, string][],
            cta: w.runtime_class === "vm" ? "Open VM" : "Open container",
            href: workloadHref(w),
        })),
        ...resourceGroups.map((rg) => ({
            id: `rg-${rg.id}`,
            group: "Resource groups",
            title: rg.name,
            subtitle: rg.internal_cidr,
            kind: "Resource group",
            icon: Layers,
            keywords: "network subnet",
            meta: [["Subnet", rg.internal_cidr], ["Status", rg.status]] as [string, string][],
            cta: "Open group",
            href: `/resource-groups/${rg.id}`,
        })),
        ...buckets.map((b) => ({
            id: `bucket-${b.id}`,
            group: "Buckets",
            title: b.name,
            subtitle: b.exposure,
            kind: "S3 bucket",
            icon: Database,
            keywords: "s3 object storage",
            meta: [["Access", b.exposure], ["Status", b.status]] as [string, string][],
            cta: "Open bucket",
            href: `/buckets/${b.id}`,
        })),
        ...alerts.map((a) => ({
            id: `alert-${a.id}`,
            group: "Alerts",
            title: a.message ?? a.event_type,
            subtitle: nodes.find((n) => n.id === a.agent_id)?.hostname ?? "",
            kind: "Alert",
            icon: TriangleAlert,
            keywords: a.event_type,
            meta: [["Severity", a.severity], ["Status", a.status]] as [string, string][],
            cta: "Open alerts",
            href: "/notifications",
        })),
    ]);

    const items = $derived([...actionItems, ...pageItems, ...resourceItems]);
    const tiles = $derived(pageItems.slice(0, TILE_COUNT));
    const hasQuery = $derived(query.trim() !== "");
    const result = $derived(search(items, query));
    const quick = $derived(suggestions(items));
    const flat = $derived(hasQuery ? result.flat : quick);
    const current = $derived(flat[Math.min(Math.max(selected, 0), flat.length - 1)] ?? pageItems[0]);

    function activate(item: PaletteItem) {
        commandPalette.hide();
        if (item.action) item.action();
        else goto(item.href);
    }

    function follow(event: MouseEvent, item: PaletteItem) {
        event.preventDefault();
        activate(item);
    }

    function move(delta: number) {
        selected = Math.max(0, Math.min(selected + delta, flat.length - 1));
        tick().then(() => list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" }));
    }

    function handleInputKey(event: KeyboardEvent) {
        if (event.key === "ArrowDown") move(1);
        else if (event.key === "ArrowUp") move(-1);
        else if (event.key === "Enter" && current) activate(current);
        else return;
        event.preventDefault();
    }

    function handleWindowKey(event: KeyboardEvent) {
        if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
            event.preventDefault();
            commandPalette.toggle();
        } else if (event.key === "Escape" && commandPalette.open) {
            commandPalette.hide();
        }
    }

    $effect(() => {
        if (!commandPalette.open) {
            query = "";
            selected = -1;
            return;
        }
        loadData();
        tick().then(() => input?.focus());
    });
</script>

<svelte:window onkeydown={handleWindowKey} />

{#snippet label(title: string)}
    {@const [before, match, after] = highlight(title, query)}
    {before}<mark>{match}</mark>{after}
{/snippet}

{#if commandPalette.open}
    <div class="cx-root sp-root">
        <button class="sp-scrim" type="button" tabindex="-1" aria-label="Close search" onclick={() => commandPalette.hide()}></button>
        <div class="sp" role="dialog" aria-modal="true" aria-label="Search csfx">
            <div class="sp-bar">
                <Search class="cx-i" aria-hidden="true" />
                <input
                    class="sp-input"
                    type="text"
                    placeholder="Search csfx"
                    aria-label="Search csfx"
                    autocomplete="off"
                    spellcheck="false"
                    bind:this={input}
                    bind:value={query}
                    oninput={() => (selected = -1)}
                    onkeydown={handleInputKey}
                />
                <button class="sp-esc" type="button" aria-label="Close search" onclick={() => commandPalette.hide()}>esc</button>
            </div>

            {#if !hasQuery}
                <div class="sp-sugg">
                    <p class="sp-h">Suggestions</p>
                    <div class="sp-tiles">
                        {#each tiles as tile (tile.id)}
                            <a class="sp-tile" href={tile.href} onclick={(event) => follow(event, tile)}>
                                <span class="sp-tile__ic"><tile.icon class="cx-i" aria-hidden="true" /></span>{tile.title}
                            </a>
                        {/each}
                    </div>
                    {#if quick.length}
                        <p class="sp-h">Quick access</p>
                        <div class="sp-list" bind:this={list}>
                            {#each quick as item, index (item.id)}
                                {@render row(item, index)}
                            {/each}
                        </div>
                    {/if}
                </div>
            {:else if flat.length}
                <div class="sp-body">
                    <div class="sp-list sp-list--scroll" role="listbox" aria-label="Results" bind:this={list}>
                        {#each result.rows as entry, position (position)}
                            {#if entry.head}
                                <p class="sp-h">{entry.label}</p>
                            {:else}
                                {@render row(entry.item, entry.index)}
                            {/if}
                        {/each}
                    </div>
                    <aside class="sp-pv" aria-label="Preview">
                        <span class="sp-pv__ic" aria-hidden="true"><current.icon class="cx-i" /></span>
                        <div>
                            <h3>{current.title}</h3>
                            <p class="sp-pv__kind">{current.kind}</p>
                        </div>
                        <dl>
                            {#each current.meta as [key, value] (key)}
                                <div><dt>{key}</dt><dd>{value}</dd></div>
                            {/each}
                        </dl>
                        <a class="cx-btn cx-btn--primary cx-btn--sm" href={current.href} style="justify-self: start" onclick={(event) => follow(event, current)}>{current.cta}</a>
                    </aside>
                </div>
            {:else}
                <div class="sp-none">
                    <b>{loading ? "Loading" : `No results for “${query.trim()}”`}</b>
                    <span>Try a node name, an IP address or a page.</span>
                </div>
            {/if}

            <div class="sp-foot">
                <span><kbd>↑</kbd><kbd>↓</kbd>navigate</span>
                <span><kbd>↵</kbd>open</span>
                <span><kbd>esc</kbd>close</span>
                <span class="sp-foot__brand">csfx search · ⌘K</span>
            </div>
        </div>
    </div>
{/if}

{#snippet row(item: PaletteItem, index: number)}
    <a class="sp-row" role="option" aria-selected={index === selected} href={item.href} onclick={(event) => follow(event, item)} onmouseenter={() => (selected = index)} onmouseleave={() => (selected = -1)}>
        <span class="sp-row__ic"><item.icon class="cx-i" aria-hidden="true" /></span>
        <span class="sp-row__t">{@render label(item.title)}</span>
        <span class="sp-row__s">{item.subtitle}</span>
    </a>
{/snippet}
