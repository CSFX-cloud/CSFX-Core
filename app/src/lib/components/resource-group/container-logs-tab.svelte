<script lang="ts">
    import { onMount, tick } from "svelte";
    import { Download, FileText, Search } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { streamWorkloadLogs } from "$lib/api/resource-groups";

    let { workloadId, running }: { workloadId: string; running: boolean } = $props();

    let lines = $state<string[]>([]);
    let error = $state<string | null>(null);
    let filter = $state("");
    let follow = $state(true);
    let terminal = $state<HTMLDivElement | null>(null);

    const visible = $derived(filter.trim() ? lines.filter((line) => line.toLowerCase().includes(filter.trim().toLowerCase())) : lines);

    async function stream(signal: AbortSignal) {
        if (!auth.token) return;
        const decoder = new TextDecoder();
        try {
            const reader = (await streamWorkloadLogs(auth.token, workloadId, signal)).getReader();
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                const chunk = decoder.decode(value, { stream: true });
                if (chunk) lines = [...lines, ...chunk.split("\n").filter((line) => line.length > 0)];
            }
        } catch (e) {
            if (e instanceof Error && e.name !== "AbortError") error = e.message;
        }
    }

    function tone(line: string): string {
        const lowered = line.toLowerCase();
        if (/\b(error|fatal|panic)\b/.test(lowered)) return "color: var(--danger)";
        return /\b(warn|warning)\b/.test(lowered) ? "color: var(--warning)" : "";
    }

    function download() {
        const url = URL.createObjectURL(new Blob([lines.join("\n")], { type: "text/plain" }));
        const link = document.createElement("a");
        link.href = url;
        link.download = `${workloadId.slice(0, 8)}-logs.txt`;
        link.click();
        URL.revokeObjectURL(url);
    }

    $effect(() => {
        visible.length;
        if (follow && terminal) tick().then(() => terminal && (terminal.scrollTop = terminal.scrollHeight));
    });

    onMount(() => {
        const controller = new AbortController();
        stream(controller.signal);
        return () => controller.abort();
    });
</script>

<section aria-label="Logs" style="display: grid; gap: 12px">
    <div class="term__bar">
        <div class="cx-input-wrap db-search">
            <Search class="cx-i" aria-hidden="true" />
            <input class="cx-input cx-input--icon" type="search" placeholder="Filter log lines" aria-label="Filter log lines" bind:value={filter} />
        </div>
        <div class="db-actions">
            <label class="cx-check"><input type="checkbox" bind:checked={follow} /> Follow</label>
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={lines.length === 0} onclick={download}>
                <Download class="cx-i" aria-hidden="true" />
                Download
            </button>
        </div>
    </div>
    <div class="term" role="log" aria-label="Container output" bind:this={terminal}>
        {#if !running}
            <div class="term__note"><FileText class="cx-i" aria-hidden="true" />The container is stopped. Showing output from its last run.</div>
        {/if}
        {#if error}<div class="term__line" style="color: var(--danger)">{error}</div>{/if}
        {#each visible as line, index (index)}
            <div class="term__line" style={tone(line)}>{line}</div>
        {/each}
        {#if running}<div class="term__line"><span class="term__cursor"></span></div>{/if}
    </div>
</section>
