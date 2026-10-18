<script lang="ts">
    import XIcon from "@lucide/svelte/icons/x";
    import CopyIcon from "@lucide/svelte/icons/copy";
    import type { LogEntry } from "$lib/api/logs";
    import { describeAction, levelTextClass } from "$lib/utils/logs";
    import SeverityBars from "./severity-bars.svelte";

    let {
        entry,
        nodeLabel,
        actorLabel,
        onclose,
    }: {
        entry: LogEntry;
        nodeLabel: (id: string | null) => string;
        actorLabel: (id: string | null) => string;
        onclose: () => void;
    } = $props();

    const rows = $derived(
        [
            ["Service", entry.service],
            ["Category", entry.classification],
            ["Node", entry.agent_id ? nodeLabel(entry.agent_id) : null],
            ["User", entry.actor_user_id ? actorLabel(entry.actor_user_id) : null],
            ["Action", entry.action],
            ["Resource", entry.target_type],
            ["Target", entry.target_id && entry.target_id !== "-" ? entry.target_id : null],
            ["Workload", entry.workload_id],
            ["Occurrences", entry.occurrences > 1 ? String(entry.occurrences) : null],
            ["Last seen", entry.occurrences > 1 ? new Date(entry.last_seen_at).toLocaleString() : null],
        ].filter((row): row is [string, string] => row[1] !== null && row[1] !== undefined),
    );
    const fieldRows = $derived(Object.entries(entry.fields ?? {}));
    const description = $derived(
        entry.kind === "user" ? `${actorLabel(entry.actor_user_id)} ${describeAction(entry)}` : entry.message,
    );

    function copyRaw() {
        navigator.clipboard.writeText(entry.cef);
    }
</script>

<div class="flex h-full flex-col gap-3 overflow-y-auto p-4">
    <div class="flex items-center justify-between">
        <span class="text-sm font-semibold">{new Date(entry.created_at).toLocaleString()}</span>
        <button type="button" class="text-muted-foreground hover:text-foreground" aria-label="Close" onclick={onclose}>
            <XIcon class="size-4" />
        </button>
    </div>

    <section class="rounded-lg bg-muted/40 p-4 text-sm">
        <div class="flex items-center justify-between py-1.5">
            <span class="text-muted-foreground">Severity</span>
            <span class="flex items-center gap-2 {levelTextClass(entry.level)}">
                {entry.level}
                <SeverityBars level={entry.level} />
            </span>
        </div>
        {#each rows as [label, value] (label)}
            <div class="flex items-center justify-between gap-4 py-1.5">
                <span class="text-muted-foreground">{label}</span>
                <span class="break-all text-right">{value}</span>
            </div>
        {/each}
        {#each fieldRows as [key, value] (key)}
            <div class="flex items-center justify-between gap-4 py-1.5">
                <span class="text-muted-foreground">{key}</span>
                <span class="break-all text-right font-mono text-xs">{String(value)}</span>
            </div>
        {/each}
    </section>

    <section class="rounded-lg bg-muted/40 p-4 text-sm">
        <h3 class="mb-2 font-semibold">Description</h3>
        <p class="whitespace-pre-wrap break-words text-muted-foreground">{description}</p>
    </section>

    <section class="rounded-lg bg-muted/40 p-4 text-sm">
        <div class="mb-2 flex items-center justify-between">
            <h3 class="font-semibold">CEF log</h3>
            <button type="button" class="text-muted-foreground hover:text-foreground" aria-label="Copy" onclick={copyRaw}>
                <CopyIcon class="size-4" />
            </button>
        </div>
        <pre class="whitespace-pre-wrap break-all rounded-md bg-background p-3 font-mono text-xs">{entry.cef}</pre>
    </section>
</div>
