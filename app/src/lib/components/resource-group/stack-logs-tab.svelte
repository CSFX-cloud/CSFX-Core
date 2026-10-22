<script lang="ts">
    import { onMount, tick } from "svelte";
    import { FileText } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { streamWorkloadLogs, type Workload } from "$lib/api/resource-groups";

    interface LogLine {
        service: string;
        text: string;
    }

    const PALETTE = ["#4ade80", "#38bdf8", "#fbbf24", "#e879f9", "#22d3ee", "#fb7185"];

    let { workloads }: { workloads: Workload[] } = $props();

    let lines = $state<LogLine[]>([]);
    let error = $state<string | null>(null);
    let follow = $state(true);
    let terminal = $state<HTMLDivElement | null>(null);

    const services = $derived(workloads.map((workload) => workload.service_name ?? workload.name));

    async function stream(workload: Workload, signal: AbortSignal) {
        if (!auth.token) return;
        const service = workload.service_name ?? workload.name;
        const decoder = new TextDecoder();
        try {
            const reader = (await streamWorkloadLogs(auth.token, workload.id, signal)).getReader();
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                const chunk = decoder.decode(value, { stream: true });
                const fresh = chunk.split("\n").filter((line) => line.length > 0).map((text) => ({ service, text }));
                if (fresh.length > 0) lines = [...lines, ...fresh];
            }
        } catch (e) {
            if (e instanceof Error && e.name !== "AbortError") error = `${service}: ${e.message}`;
        }
    }

    $effect(() => {
        lines.length;
        if (follow && terminal) tick().then(() => terminal && (terminal.scrollTop = terminal.scrollHeight));
    });

    onMount(() => {
        const controller = new AbortController();
        workloads.forEach((workload) => stream(workload, controller.signal));
        return () => controller.abort();
    });
</script>

<section aria-label="Stack logs" style="display: grid; gap: 12px">
    <div class="term__bar">
        <span class="cx-hint">{services.length} services streaming</span>
        <label class="cx-check"><input type="checkbox" bind:checked={follow} /> Follow</label>
    </div>
    <div class="term" role="log" aria-label="Stack output" bind:this={terminal}>
        {#if error}<div class="term__line" style="color: var(--danger)">{error}</div>{/if}
        {#if lines.length === 0 && !error}
            <div class="term__note"><FileText class="cx-i" aria-hidden="true" />Waiting for output from the services.</div>
        {/if}
        {#each lines as line, index (index)}
            <div class="term__line">
                <b style="color: {PALETTE[services.indexOf(line.service) % PALETTE.length]}">{line.service}</b>
                <span style="color: var(--marker)">|</span>
                {line.text}
            </div>
        {/each}
    </div>
</section>
