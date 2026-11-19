<script lang="ts">
    import { onMount } from "svelte";
    import { listLogs, type LogEntry, type LogsFilter } from "$lib/api/logs";
    import { levelMeta } from "$lib/utils/logs";

    const SKELETON_ROWS = 4;
    const TIME_START = 11;
    const TIME_END = 19;

    let { filter, href }: { filter: LogsFilter; href?: string } = $props();

    let entries = $state<LogEntry[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);

    onMount(async () => {
        try {
            entries = (await listLogs(filter)).entries;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load logs";
        } finally {
            loading = false;
        }
    });

    function time(entry: LogEntry): string {
        return entry.last_seen_at.slice(TIME_START, TIME_END);
    }
</script>

<section class="tp-logs" aria-label="Recent logs">
    <div class="tp-logs__head">
        <h3>Recent logs</h3>
        {#if href}<a class="cx-link" {href}>View all</a>{/if}
    </div>
    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: SKELETON_ROWS } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if entries.length === 0}
        <p class="cx-hint">No log entries yet.</p>
    {:else}
        <table class="cx-table">
            <thead><tr><th>Level</th><th>Message</th><th>Time</th></tr></thead>
            <tbody>
                {#each entries as entry (entry.id)}
                    {@const meta = levelMeta(entry.level)}
                    <tr>
                        <td><span class="cx-badge {meta.tone ? `cx-badge--${meta.tone}` : ''}">{meta.label}</span></td>
                        <td class="tp-logs__msg" title={entry.message}>{entry.message}</td>
                        <td class="cx-num">{time(entry)}</td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {/if}
</section>
