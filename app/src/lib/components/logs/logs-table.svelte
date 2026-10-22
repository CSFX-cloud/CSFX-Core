<script lang="ts">
    import { FileText } from "@lucide/svelte";
    import type { LogEntry } from "$lib/api/logs";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { describeAction, levelMeta } from "$lib/utils/logs";

    const SKELETON_ROWS = 6;

    type Props = {
        kind: "user" | "system";
        entries: LogEntry[];
        selectedId: string | null;
        loading: boolean;
        offset: number;
        total: number;
        pageLimit: number;
        hasMore: boolean;
        nodeLabel: (id: string | null) => string;
        actorLabel: (id: string | null) => string;
        onselect: (entry: LogEntry) => void;
        onpage: (offset: number) => void;
    };

    let { kind, entries, selectedId, loading, offset, total, pageLimit, hasMore, nodeLabel, actorLabel, onselect, onpage }: Props = $props();

    const summary = $derived(entries.length === 0 ? "No events match these filters" : `${(offset + 1).toLocaleString()}–${(offset + entries.length).toLocaleString()} of ${total.toLocaleString()} events`);

    function time(value: string): string {
        return value.slice(0, 19).replace("T", " ");
    }

    function target(entry: LogEntry): string {
        return !entry.target_id || entry.target_id === "-" ? "—" : entry.target_id;
    }
</script>

<section class="cx-panel lg-panel" aria-label="Log events">
    {#if loading}
        <div class="db-skeleton-list" style="padding: 16px" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if entries.length === 0}
        <EmptyState icon={FileText} label="No log entries" />
    {:else}
        <div class="db-scroll">
            <table class="cx-table lg-table">
                <thead>
                    {#if kind === "system"}
                        <tr><th>Severity</th><th>Category</th><th>Service</th><th>Node</th><th>Message</th><th>Time</th></tr>
                    {:else}
                        <tr><th>Time</th><th>User</th><th>Event</th><th>Action</th><th>Resource</th><th>Target</th></tr>
                    {/if}
                </thead>
                <tbody>
                    {#each entries as entry (entry.id)}
                        {@const meta = levelMeta(entry.level)}
                        <tr class="nl-row {selectedId === entry.id ? 'lg-row--selected' : ''}">
                            {#if kind === "system"}
                                <td><span class="cx-badge {meta.tone ? `cx-badge--${meta.tone}` : ''}">{meta.label}</span></td>
                                <td class="cx-soft">{entry.classification}</td>
                                <td class="cx-soft">{entry.service}</td>
                                <td class="cx-soft">{nodeLabel(entry.agent_id)}</td>
                                <td class="msg {meta.tone === 'danger' ? 'err' : ''}">
                                    <button class="rs-link lg-msg" type="button" onclick={() => onselect(entry)}>{entry.message}</button>
                                    {#if entry.occurrences > 1}<span class="rs-restarts">x{entry.occurrences}</span>{/if}
                                </td>
                                <td class="cx-num">{time(entry.last_seen_at)}</td>
                            {:else}
                                <td class="cx-num">{time(entry.created_at)}</td>
                                <td class="cx-soft">{actorLabel(entry.actor_user_id)}</td>
                                <td class="msg"><button class="rs-link lg-msg" type="button" onclick={() => onselect(entry)}>{describeAction(entry)}</button></td>
                                <td class="cx-num">{entry.action ?? entry.message}</td>
                                <td class="cx-soft">{entry.target_type ?? "—"}</td>
                                <td class="cx-num">{target(entry)}</td>
                            {/if}
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
    <div class="lg-foot">
        <span>{loading ? "Loading events" : summary}</span>
        <div class="db-actions">
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={offset === 0} onclick={() => onpage(Math.max(0, offset - pageLimit))}>Previous</button>
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={!hasMore} onclick={() => onpage(offset + pageLimit)}>Next</button>
        </div>
    </div>
</section>
