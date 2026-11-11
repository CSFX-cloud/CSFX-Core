<script lang="ts">
    import { onMount, tick } from "svelte";
    import { Download } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getDeployment, type Deployment } from "$lib/api/sites";
    import { shortSha } from "$lib/utils/site-status";

    const POLL_INTERVAL_MS = 2000;

    type Props = { rgId: string; siteId: string; deploymentId: string; onSettled?: () => void };

    let { rgId, siteId, deploymentId, onSettled }: Props = $props();

    let deployment = $state<Deployment | null>(null);
    let error = $state<string | null>(null);
    let terminal = $state<HTMLDivElement | null>(null);

    const lines = $derived(deployment ? deployment.build_log.split("\n").filter((line) => line !== "") : []);
    const running = $derived(deployment?.status === "queued" || deployment?.status === "building");

    function tone(line: string): string {
        if (line.startsWith("[ERROR]")) return "color: var(--danger)";
        if (line.startsWith("[WARN]")) return "color: var(--warning)";
        return "";
    }

    async function refresh() {
        if (!auth.token) return;
        const wasRunning = running;
        try {
            deployment = await getDeployment(auth.token, rgId, siteId, deploymentId);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load the build log";
        }
        if (wasRunning && !running) onSettled?.();
        await tick();
        if (terminal && running) terminal.scrollTop = terminal.scrollHeight;
    }

    function download() {
        const url = URL.createObjectURL(new Blob([deployment?.build_log ?? ""], { type: "text/plain" }));
        const link = document.createElement("a");
        link.href = url;
        link.download = `build-${deploymentId.slice(0, 8)}.txt`;
        link.click();
        URL.revokeObjectURL(url);
    }

    onMount(() => {
        refresh();
        const interval = setInterval(() => {
            if (running || !deployment) refresh();
        }, POLL_INTERVAL_MS);
        return () => clearInterval(interval);
    });
</script>

<section class="cx-panel" aria-labelledby="bl-title">
    <div class="cx-panel__head" style="align-items: center">
        <div>
            <h2 class="cx-panel__title" id="bl-title">Build log</h2>
            <p class="cx-panel__sub">
                {#if deployment}{shortSha(deployment.commit_sha)} &middot; {deployment.branch} &middot; {deployment.status.replace("_", " ")}{:else}Loading{/if}
            </p>
        </div>
        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={lines.length === 0} onclick={download}>
            <Download class="cx-i" aria-hidden="true" />
            Download
        </button>
    </div>
    <div class="term" role="log" aria-label="Build output" bind:this={terminal}>
        {#if error}<div class="term__line" style="color: var(--danger)">{error}</div>{/if}
        {#each lines as line, index (index)}
            <div class="term__line" style={tone(line)}>{line}</div>
        {:else}
            {#if !error}<div class="term__line">{running ? "Waiting for the build to start" : "No output"}</div>{/if}
        {/each}
        {#if running}<div class="term__line"><span class="term__cursor"></span></div>{/if}
    </div>
</section>
