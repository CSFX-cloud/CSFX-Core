<script lang="ts">
    import { Copy, X } from "@lucide/svelte";
    import type { LogEntry } from "$lib/api/logs";
    import { describeAction, levelMeta } from "$lib/utils/logs";

    type Props = {
        entry: LogEntry;
        nodeLabel: (id: string | null) => string;
        actorLabel: (id: string | null) => string;
        onclose: () => void;
    };

    let { entry, nodeLabel, actorLabel, onclose }: Props = $props();

    const meta = $derived(levelMeta(entry.level));
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
    const description = $derived(entry.kind === "user" ? `${actorLabel(entry.actor_user_id)} ${describeAction(entry)}` : entry.message);

    function copyRaw() {
        navigator.clipboard.writeText(entry.cef);
    }
</script>

<aside class="lg-detail" aria-label="Log entry">
    <div class="lg-detail__head">
        <span class="cx-num" style="color: var(--ink)">{new Date(entry.created_at).toLocaleString()}</span>
        <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Close" onclick={onclose}>
            <X class="cx-i" aria-hidden="true" />
        </button>
    </div>

    <section class="cx-panel lg-detail__panel">
        <dl class="nd-kv">
            <div><dt>Severity</dt><dd><span class="cx-badge {meta.tone ? `cx-badge--${meta.tone}` : ''}">{meta.label}</span></dd></div>
            {#each rows as [label, value] (label)}
                <div><dt>{label}</dt><dd style="word-break: break-all">{value}</dd></div>
            {/each}
            {#each fieldRows as [key, value] (key)}
                <div><dt>{key}</dt><dd class="mono" style="word-break: break-all">{String(value)}</dd></div>
            {/each}
        </dl>
    </section>

    <section class="cx-panel lg-detail__panel">
        <h3 class="cx-panel__title" style="margin-bottom: 8px">Description</h3>
        <p class="lg-detail__text">{description}</p>
    </section>

    <section class="cx-panel lg-detail__panel">
        <div class="cx-panel__head" style="margin-bottom: 8px; align-items: center">
            <h3 class="cx-panel__title">CEF log</h3>
            <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Copy CEF log" onclick={copyRaw}>
                <Copy class="cx-i" aria-hidden="true" />
            </button>
        </div>
        <pre class="lg-detail__raw">{entry.cef}</pre>
    </section>
</aside>
