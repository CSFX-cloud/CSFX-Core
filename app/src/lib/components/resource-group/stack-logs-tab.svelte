<script lang="ts">
    import { onMount } from "svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { streamWorkloadLogs, type Workload } from "$lib/api/resource-groups";

    interface LogLine {
        service: string;
        text: string;
    }

    const PALETTE = ["text-green-400", "text-sky-400", "text-amber-400", "text-fuchsia-400", "text-cyan-400", "text-rose-400"];

    let { workloads }: { workloads: Workload[] } = $props();

    let lines = $state<LogLine[]>([]);
    let error = $state<string | null>(null);

    const services = $derived(workloads.map((w) => w.service_name ?? w.name));

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
                const fresh = chunk.split("\n").filter((l) => l.length > 0).map((text) => ({ service, text }));
                if (fresh.length > 0) lines = [...lines, ...fresh];
            }
        } catch (e) {
            if (e instanceof Error && e.name !== "AbortError") error = `${service}: ${e.message}`;
        }
    }

    onMount(() => {
        const controller = new AbortController();
        workloads.forEach((workload) => stream(workload, controller.signal));
        return () => controller.abort();
    });
</script>

<div class="h-full overflow-y-auto bg-black p-4 font-mono text-xs">
    {#if error}
        <p class="text-red-400">{error}</p>
    {/if}
    {#each lines as line}
        <p class="whitespace-pre-wrap text-green-400">
            <span class="{PALETTE[services.indexOf(line.service) % PALETTE.length]} font-semibold">{line.service}</span>
            <span class="text-muted-foreground">|</span>
            {line.text}
        </p>
    {/each}
</div>
