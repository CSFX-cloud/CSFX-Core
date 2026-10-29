<script lang="ts">
    import { onMount } from "svelte";
    import { Cpu, HardDrive, MemoryStick, Server, ServerOff, TriangleAlert } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { listEvents, resolveEvent, type AlertEvent } from "$lib/api/events";
    import { notifications } from "$lib/components/shell/notifications.svelte";

    type Inbox = "all" | "open" | "resolved";
    type Source = "all" | "cluster" | "resources";

    const SEVERITIES = ["critical", "warning", "info"] as const;
    const SEVERITY_BADGE: Record<string, string> = { critical: "cx-badge--danger", warning: "cx-badge--warning", info: "" };
    const EVENT_ICONS = { high_cpu: Cpu, high_memory: MemoryStick, high_disk: HardDrive, degraded: Server, offline: ServerOff, online: Server } as const;
    const SOURCES: { value: Source; label: string }[] = [
        { value: "all", label: "Everything" },
        { value: "cluster", label: "Cluster and nodes" },
        { value: "resources", label: "Resource usage" },
    ];
    const INBOXES: { value: Inbox; label: string }[] = [
        { value: "all", label: "All" },
        { value: "open", label: "Open" },
        { value: "resolved", label: "Resolved" },
    ];

    let events = $state<AlertEvent[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let inbox = $state<Inbox>("all");
    let source = $state<Source>("all");
    let severities = $state<Record<string, boolean>>({ critical: true, warning: true, info: true });
    let query = $state("");
    let selectedId = $state<string | null>(null);
    let resolving = $state(false);

    const timeFormat = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" });
    const dateFormat = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });

    const sourceOf = (event: AlertEvent): Source => (event.event_type.startsWith("high_") ? "resources" : "cluster");
    const titleOf = (event: AlertEvent) => event.event_type.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());
    const textOf = (event: AlertEvent) => event.message ?? titleOf(event);
    const iconOf = (event: AlertEvent) => EVENT_ICONS[event.event_type as keyof typeof EVENT_ICONS] ?? TriangleAlert;
    const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();

    function dayOf(event: AlertEvent): string {
        const diff = Math.round((startOfDay(new Date()) - startOfDay(new Date(event.created_at))) / 86_400_000);
        if (diff <= 0) return "Today";
        return diff === 1 ? "Yesterday" : "Earlier";
    }

    function matches(event: AlertEvent): boolean {
        if (inbox !== "all" && event.status !== inbox) return false;
        if (source !== "all" && sourceOf(event) !== source) return false;
        if (!severities[event.severity]) return false;
        return textOf(event).toLowerCase().includes(query.trim().toLowerCase()) || titleOf(event).toLowerCase().includes(query.trim().toLowerCase());
    }

    const visible = $derived(events.filter(matches));
    const resolvable = $derived(visible.filter((event) => event.status === "open"));
    const selected = $derived(events.find((event) => event.id === selectedId) ?? null);
    const openCount = $derived(events.filter((event) => event.status === "open").length);
    const criticalCount = $derived(events.filter((event) => event.status === "open" && event.severity === "critical").length);
    const summary = $derived(openCount ? `${openCount} open · ${criticalCount} critical` : "You are all caught up.");
    const sourceCount = (value: Source) => events.filter((event) => value === "all" || sourceOf(event) === value).length;

    async function load() {
        if (!auth.token) return;
        try {
            events = await listEvents(auth.token);
            notifications.set(events.filter((event) => event.status === "open").length);
            selectedId = selectedId ?? events[0]?.id ?? null;
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to load notifications";
        } finally {
            loading = false;
        }
    }

    async function resolve(event: AlertEvent) {
        if (!auth.token) return;
        resolving = true;
        try {
            await resolveEvent(auth.token, event.id);
            await load();
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to resolve notification";
        } finally {
            resolving = false;
        }
    }

    async function resolveAll() {
        if (!auth.token) return;
        const token = auth.token;
        resolving = true;
        try {
            await Promise.all(resolvable.map((event) => resolveEvent(token, event.id)));
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to resolve notifications";
        } finally {
            await load();
            resolving = false;
        }
    }

    onMount(load);
</script>

<div class="nt-main">
    <aside class="nt-rail" aria-label="Filters">
        <div>
            <h2>Inbox</h2>
            <div class="cx-tabs cx-tabs--contained cx-tabs--sm cx-tabs--block" role="tablist" aria-label="Inbox">
                {#each INBOXES as box (box.value)}
                    <button class="cx-tab" role="tab" type="button" style="padding: 0 8px" aria-selected={inbox === box.value} onclick={() => (inbox = box.value)}>
                        {box.label}
                    </button>
                {/each}
            </div>
        </div>
        <div>
            <h2>Severity</h2>
            <div class="nt-sev" role="group" aria-label="Severity">
                {#each SEVERITIES as severity (severity)}
                    <button type="button" aria-pressed={severities[severity]} onclick={() => (severities[severity] = !severities[severity])}>
                        <i class="nt-d-{severity}"></i>{severity.replace(/^\w/, (c) => c.toUpperCase())}
                    </button>
                {/each}
            </div>
        </div>
        <div>
            <h2>Source</h2>
            <div class="nt-src" role="group" aria-label="Source">
                {#each SOURCES as option (option.value)}
                    <button type="button" aria-pressed={source === option.value} onclick={() => (source = option.value)}>
                        <i class="nt-c-{option.value}"></i>{option.label}<em>{sourceCount(option.value)}</em>
                    </button>
                {/each}
            </div>
        </div>
        <div style="margin-top: auto">
            <a class="cx-btn cx-btn--ghost cx-btn--sm" href="/admin/settings/notifications" style="justify-content: flex-start; padding: 0">Notification settings</a>
        </div>
    </aside>

    <div class="nt-body">
        <div class="nt-head">
            <div>
                <h1>Notifications</h1>
                <p>{summary}</p>
            </div>
            <div class="db-actions">
                <div class="cx-input-wrap db-search" style="width: 260px">
                    <input class="cx-input" type="search" placeholder="Search notifications" aria-label="Search notifications" bind:value={query} />
                </div>
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={resolving || !resolvable.length} aria-busy={resolving} onclick={resolveAll}>
                    Resolve all{resolvable.length ? ` (${resolvable.length})` : ""}
                </button>
            </div>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <div class="nt-split">
            <section class="cx-panel nt-list" aria-label="Notification list" style="padding: 0">
                {#if loading}
                    <div class="db-skeleton db-skeleton--row" style="margin: 20px" aria-hidden="true"></div>
                {:else}
                    {#each visible as event, index (event.id)}
                        {@const Icon = iconOf(event)}
                        {#if index === 0 || dayOf(visible[index - 1]) !== dayOf(event)}<div class="nt-day">{dayOf(event)}</div>{/if}
                        <button class="nt-item" class:is-sel={event.id === selectedId} class:is-read={event.status !== "open"} type="button" aria-pressed={event.id === selectedId} onclick={() => (selectedId = event.id)}>
                            <span class="nt-ic" aria-hidden="true"><Icon /></span>
                            <span style="min-width: 0">
                                <span class="nt-top"><span class="cx-badge {SEVERITY_BADGE[event.severity] ?? ''}">{event.severity}</span><b>{titleOf(event)}</b></span>
                                <span class="nt-text">{textOf(event)}</span>
                            </span>
                            <span class="nt-meta">{timeFormat.format(new Date(event.created_at))}{#if event.status === "open"}<span class="nt-dot" aria-label="Open"></span>{/if}</span>
                        </button>
                    {:else}
                        <p class="nt-none">Nothing here. You're all caught up.</p>
                    {/each}
                {/if}
            </section>

            {#if selected}
                {@const Icon = iconOf(selected)}
                <aside class="cx-panel nt-detail" aria-label="Notification details">
                    <div class="nt-detail__head">
                        <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px">
                            <span class="nt-ic nt-ic--lg" aria-hidden="true"><Icon /></span>
                            <span class="cx-badge {SEVERITY_BADGE[selected.severity] ?? ''}">{selected.severity}</span>
                        </div>
                        <h2>{titleOf(selected)}</h2>
                        <p>{textOf(selected)}</p>
                    </div>
                    <div class="db-actions">
                        {#if selected.agent_id}<a class="cx-btn cx-btn--primary cx-btn--sm" href="/nodes/{selected.agent_id}">Open node</a>{/if}
                        {#if selected.status === "open"}
                            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={resolving} aria-busy={resolving} onclick={() => resolve(selected)}>Resolve</button>
                        {/if}
                    </div>
                    <div class="nt-sec">
                        <h3>Details</h3>
                        <dl class="nt-kv">
                            <div><dt>Type</dt><dd>{selected.event_type}</dd></div>
                            <div><dt>Status</dt><dd>{selected.status}</dd></div>
                            <div><dt>Affected workloads</dt><dd>{selected.affected_workloads?.length ?? 0}</dd></div>
                            <div><dt>First seen</dt><dd>{dateFormat.format(new Date(selected.created_at))}</dd></div>
                        </dl>
                    </div>
                    <div class="nt-sec">
                        <h3>Timeline</h3>
                        <ol class="nt-tl">
                            {#if selected.resolved_at}<li>Resolved<span>{dateFormat.format(new Date(selected.resolved_at))}</span></li>{/if}
                            <li>Raised<span>{dateFormat.format(new Date(selected.created_at))}</span></li>
                        </ol>
                    </div>
                </aside>
            {/if}
        </div>
    </div>
</div>
