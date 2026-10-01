<script lang="ts">
    import { onMount } from "svelte";
    import type { Terminal } from "@xterm/xterm";
    import "@xterm/xterm/css/xterm.css";
    import { auth } from "$lib/auth/store.svelte";
    import { openWorkloadExecSocket, type Workload } from "$lib/api/resource-groups";

    let { workload }: { workload: Workload } = $props();

    let error = $state<string | null>(null);
    let terminalElement = $state<HTMLDivElement | null>(null);

    const running = $derived(workload.status === "running");

    async function startSession(element: HTMLDivElement, token: string): Promise<() => void> {
        const { Terminal } = await import("@xterm/xterm");
        const { FitAddon } = await import("@xterm/addon-fit");

        const terminal: Terminal = new Terminal({ convertEol: true, cursorBlink: true });
        const fitAddon = new FitAddon();
        terminal.loadAddon(fitAddon);
        terminal.open(element);
        fitAddon.fit();

        let socket: WebSocket | null = null;
        try {
            socket = await openWorkloadExecSocket(token, workload.id);
            socket.binaryType = "arraybuffer";
            socket.onmessage = (event) =>
                terminal.write(event.data instanceof ArrayBuffer ? new Uint8Array(event.data) : event.data);
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

<div class="flex flex-col h-full">
    {#if !running}
        <p class="p-4 text-xs text-muted-foreground">Shell is only available while the container is running.</p>
    {:else}
        {#if error}
            <p class="px-4 py-2 text-xs text-destructive shrink-0">{error}</p>
        {/if}
        <div class="flex-1 overflow-hidden bg-black p-2" bind:this={terminalElement}></div>
    {/if}
</div>
