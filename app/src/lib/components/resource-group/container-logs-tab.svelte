<script lang="ts">
    import { onMount } from "svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { streamWorkloadLogs } from "$lib/api/resource-groups";

    let { workloadId }: { workloadId: string } = $props();

    let lines = $state<string[]>([]);
    let error = $state<string | null>(null);

    async function stream(signal: AbortSignal) {
        if (!auth.token) return;
        const decoder = new TextDecoder();
        try {
            const body = await streamWorkloadLogs(auth.token, workloadId, signal);
            const reader = body.getReader();
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                const chunk = decoder.decode(value, { stream: true });
                if (chunk) lines = [...lines, ...chunk.split("\n").filter((l) => l.length > 0)];
            }
        } catch (e) {
            if (e instanceof Error && e.name !== "AbortError") error = e.message;
        }
    }

    onMount(() => {
        const controller = new AbortController();
        stream(controller.signal);
        return () => controller.abort();
    });
</script>

<div class="h-full overflow-y-auto bg-black p-4 font-mono text-xs text-green-400">
    {#if error}
        <p class="text-red-400">{error}</p>
    {/if}
    {#each lines as line}
        <p class="whitespace-pre-wrap">{line}</p>
    {/each}
</div>
