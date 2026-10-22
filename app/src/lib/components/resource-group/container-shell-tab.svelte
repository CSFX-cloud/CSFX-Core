<script lang="ts">
    import { onMount } from "svelte";
    import { Play, Terminal as TerminalIcon } from "@lucide/svelte";
    import type { Terminal } from "@xterm/xterm";
    import "@xterm/xterm/css/xterm.css";
    import { auth } from "$lib/auth/store.svelte";
    import { openWorkloadExecSocket, type Workload } from "$lib/api/resource-groups";

    let { workload, onstart }: { workload: Workload; onstart: () => void } = $props();

    let error = $state<string | null>(null);
    let terminalElement = $state<HTMLDivElement | null>(null);

    const running = $derived(workload.status === "running");

    async function startSession(element: HTMLDivElement, token: string): Promise<() => void> {
        const { Terminal } = await import("@xterm/xterm");
        const { FitAddon } = await import("@xterm/addon-fit");

        const terminal: Terminal = new Terminal({ convertEol: true, cursorBlink: true, theme: { background: "#070707" } });
        const fitAddon = new FitAddon();
        terminal.loadAddon(fitAddon);
        terminal.open(element);
        fitAddon.fit();

        let socket: WebSocket | null = null;
        try {
            socket = await openWorkloadExecSocket(token, workload.id);
            socket.binaryType = "arraybuffer";
            socket.onmessage = (event) => terminal.write(event.data instanceof ArrayBuffer ? new Uint8Array(event.data) : event.data);
            socket.onerror = () => (error = "Exec socket error");
            socket.onclose = () => terminal.write("\r\n[session closed]\r\n");
            terminal.onData((data) => socket?.send(data));
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to start exec session";
        }

        return () => {
            socket?.close();
            terminal.dispose();
        };
    }

    onMount(() => {
        if (!auth.token || !terminalElement || !running) return;
        const session = startSession(terminalElement, auth.token);
        return () => {
            session.then((cleanup) => cleanup());
        };
    });
</script>

{#if !running}
    <section class="cx-stage empty" aria-label="Shell unavailable">
        <div class="cx-stage__canvas"></div>
        <div class="empty__in">
            <span class="glow" aria-hidden="true"><TerminalIcon class="cx-i" /></span>
            <h2>Start the container to open a shell.</h2>
            <p>The shell runs inside {workload.service_name ?? workload.name} and closes when it stops.</p>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" style="margin-top: 8px" onclick={onstart}>
                <Play class="cx-i" aria-hidden="true" />
                Start container
            </button>
        </div>
    </section>
{:else}
    {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
    <div class="term term--fill" aria-label="Shell">
        <div bind:this={terminalElement}></div>
    </div>
{/if}
