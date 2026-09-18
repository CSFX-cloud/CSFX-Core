<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { auth } from '$lib/auth/store.svelte';
    import { getNode, getNodeMetricsLatest, openNodeMetricsSocket, rebootNode, powerOffNode, drainNode, uncordonNode, type LiveNodeMetrics, type Node, type NodeMetricsLatest } from '$lib/api/nodes';
    import * as Sidebar from '$lib/components/ui/sidebar/index.js';
    import { Button } from '$lib/components/ui/button/index.js';
    import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';

    const nodeId: string = $page.params.id;

    let node = $state<Node | null>(null);
    let loading = $state(true);
    let error = $state<string | null>(null);

    let metrics = $state<NodeMetricsLatest | null>(null);
    let metricsError = $state<string | null>(null);
    let metricsLoading = $state(false);
    let metricsLive = $state(false);
    let activeTab = $state<'summary' | 'hardware' | 'workloads' | 'network' | 'tasks'>('summary');
    let liveSocket: WebSocket | null = null;
    let powerActionBusy = $state(false);
    let powerActionError = $state<string | null>(null);

    async function loadNode() {
        if (!auth.token) return;
        loading = true;
        try {
            node = await getNode(auth.token, nodeId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load node';
        } finally {
            loading = false;
        }
    }

    let loadStarted = false;

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            loadNode().then(() => {
                loadMetrics(nodeId).then(() => openLiveMetrics(nodeId));
            });
        }

        return () => closeLiveMetrics();
    });

    async function loadMetrics(id: string) {
        if (!auth.token) return;
        metricsLoading = true;
        try {
            metrics = await getNodeMetricsLatest(auth.token, id);
        } catch {
            metricsError = 'No metrics available';
        } finally {
            metricsLoading = false;
        }
    }

    function applyLiveSample(sample: LiveNodeMetrics) {
        const timestamp = new Date().toISOString();
        metrics = {
            id: metrics?.id ?? '',
            agent_id: metrics?.agent_id ?? '',
            timestamp,
            cpu_model: metrics?.cpu_model ?? null,
            cpu_cores: sample.cpu_cores,
            cpu_threads: metrics?.cpu_threads ?? null,
            cpu_usage_percent: sample.cpu_usage_percent,
            memory_total_bytes: sample.memory_total_bytes,
            memory_used_bytes: sample.memory_used_bytes,
            memory_usage_percent: sample.memory_total_bytes > 0
                ? (sample.memory_used_bytes / sample.memory_total_bytes) * 100
                : null,
            disk_total_bytes: sample.disk_total_bytes,
            disk_used_bytes: sample.disk_used_bytes,
            disk_usage_percent: sample.disk_total_bytes > 0
                ? (sample.disk_used_bytes / sample.disk_total_bytes) * 100
                : null,
            network_rx_bytes: sample.network_rx_bytes,
            network_tx_bytes: sample.network_tx_bytes,
            os_name: metrics?.os_name ?? null,
            os_version: metrics?.os_version ?? null,
            kernel_version: metrics?.kernel_version ?? null,
            hostname: metrics?.hostname ?? null,
            uptime_seconds: sample.uptime_seconds,
        };
    }

    async function openLiveMetrics(agentId: string) {
        if (!auth.token) return;
        try {
            const socket = await openNodeMetricsSocket(auth.token, agentId);
            liveSocket = socket;
            socket.onmessage = (event) => {
                try {
                    applyLiveSample(JSON.parse(event.data) as LiveNodeMetrics);
                    metricsLive = true;
                } catch {}
            };
            socket.onclose = () => { metricsLive = false; };
            socket.onerror = () => { metricsLive = false; };
        } catch {
            metricsLive = false;
        }
    }

    function closeLiveMetrics() {
        liveSocket?.close();
        liveSocket = null;
        metricsLive = false;
    }

    function bytesToGb(bytes: number | null): string {
        if (bytes == null) return '-';
        return (bytes / 1_073_741_824).toFixed(1) + ' GB';
    }

    function formatUptime(seconds: number | null): string {
        if (seconds == null) return '-';
        const d = Math.floor(seconds / 86400);
        const h = Math.floor((seconds % 86400) / 3600);
        const m = Math.floor((seconds % 3600) / 60);
        if (d > 0) return `${d} days, ${h}h ${m}m`;
        if (h > 0) return `${h}h ${m}m`;
        return `${m}m`;
    }

    function clampPct(value: number | null): number {
        if (value == null) return 0;
        return Math.min(Math.max(value, 0), 100);
    }

    function pctStr(value: number | null): string {
        if (value == null) return '-';
        return value.toFixed(1) + '%';
    }

    function memPct(): number | null {
        if (metrics?.memory_usage_percent != null) return metrics.memory_usage_percent;
        if (metrics?.memory_total_bytes != null && metrics?.memory_used_bytes != null && metrics.memory_total_bytes > 0) {
            return (metrics.memory_used_bytes / metrics.memory_total_bytes) * 100;
        }
        return null;
    }

    function diskPct(): number | null {
        if (metrics?.disk_usage_percent != null) return metrics.disk_usage_percent;
        if (metrics?.disk_total_bytes != null && metrics?.disk_used_bytes != null && metrics.disk_total_bytes > 0) {
            return (metrics.disk_used_bytes / metrics.disk_total_bytes) * 100;
        }
        return null;
    }

    function netRxGb(): number {
        return (metrics?.network_rx_bytes ?? 0) / 1_073_741_824;
    }

    function netTxGb(): number {
        return (metrics?.network_tx_bytes ?? 0) / 1_073_741_824;
    }

    function netRxPct(): number {
        return Math.min((netRxGb() / 100) * 100, 100);
    }

    function netTxPct(): number {
        return Math.min((netTxGb() / 100) * 100, 100);
    }

    function formatBytes(bytes: number | null): string {
        if (bytes == null) return '-';
        if (bytes >= 1_073_741_824) return (bytes / 1_073_741_824).toFixed(1) + ' GB';
        if (bytes >= 1_048_576) return (bytes / 1_048_576).toFixed(1) + ' MB';
        return (bytes / 1024).toFixed(1) + ' KB';
    }

    function statusDotClass(status: string): string {
        switch (status.toLowerCase()) {
            case 'online': return 'bg-green-500';
            case 'offline': return 'bg-red-500';
            case 'degraded': return 'bg-yellow-500';
            default: return 'bg-muted-foreground';
        }
    }

    function gaugeArc(value: number, radius: number): { dasharray: string; circumference: number } {
        const circumference = 2 * Math.PI * radius;
        const filled = (value / 100) * circumference;
        return { dasharray: `${filled.toFixed(1)} ${circumference.toFixed(1)}`, circumference };
    }

    function gaugeColor(value: number): string {
        if (value > 80) return '#ef4444';
        if (value > 60) return '#eab308';
        return 'currentColor';
    }

    function refreshedLabel(timestamp: string): string {
        const diff = Math.floor((Date.now() - new Date(timestamp).getTime()) / 1000);
        return diff < 60 ? `refreshed ${diff}s ago` : `refreshed ${Math.floor(diff / 60)}m ago`;
    }

    async function handleReboot() {
        if (!auth.token || !node) return;
        if (!confirm(`Reboot ${node.hostname}? This will restart the host and briefly disconnect all workloads on it.`)) return;
        powerActionBusy = true;
        powerActionError = null;
        try {
            await rebootNode(auth.token, node.id);
        } catch (e) {
            powerActionError = e instanceof Error ? e.message : 'Failed to reboot node';
        } finally {
            powerActionBusy = false;
        }
    }

    async function handlePowerOff() {
        if (!auth.token || !node) return;
        if (!confirm(`Power off ${node.hostname}? The host will shut down and stop all workloads on it.`)) return;
        powerActionBusy = true;
        powerActionError = null;
        try {
            await powerOffNode(auth.token, node.id);
        } catch (e) {
            powerActionError = e instanceof Error ? e.message : 'Failed to power off node';
        } finally {
            powerActionBusy = false;
        }
    }

    async function handleDrain() {
        if (!auth.token || !node) return;
        if (!confirm(`Drain ${node.hostname}? All workloads will be rescheduled to other nodes and this node will stop receiving new ones.`)) return;
        powerActionBusy = true;
        powerActionError = null;
        try {
            await drainNode(auth.token, node.id);
            node.cordoned = true;
        } catch (e) {
            powerActionError = e instanceof Error ? e.message : 'Failed to drain node';
        } finally {
            powerActionBusy = false;
        }
    }

    async function handleUncordon() {
        if (!auth.token || !node) return;
        powerActionBusy = true;
        powerActionError = null;
        try {
            await uncordonNode(auth.token, node.id);
            node.cordoned = false;
        } catch (e) {
            powerActionError = e instanceof Error ? e.message : 'Failed to uncordon node';
        } finally {
            powerActionBusy = false;
        }
    }

    const tabs: { id: typeof activeTab; label: string }[] = [
        { id: 'summary', label: 'Summary' },
        { id: 'hardware', label: 'Hardware' },
        { id: 'workloads', label: 'Workloads' },
        { id: 'network', label: 'Network' },
        { id: 'tasks', label: 'Tasks' },
    ];
</script>

{#snippet gaugeCard(value: number, label: string, detail: string)}
    {#if true}
        {@const arc = gaugeArc(value, 20)}
        <div class="border rounded-lg p-3 flex items-center gap-3">
            <div class="relative shrink-0">
                <svg width="52" height="52" viewBox="0 0 52 52" style="color: {gaugeColor(value)}">
                    <circle cx="26" cy="26" r="20" fill="none" stroke="currentColor" stroke-width="4" stroke-opacity="0.12"/>
                    <circle
                        cx="26" cy="26" r="20" fill="none"
                        stroke="currentColor" stroke-width="4"
                        stroke-dasharray={arc.dasharray}
                        stroke-linecap="round"
                        transform="rotate(-180 26 26)"
                    />
                </svg>
                <span class="absolute inset-0 flex items-center justify-center text-xs font-semibold">{value.toFixed(0)}%</span>
            </div>
            <div class="min-w-0">
                <p class="text-xs font-medium">{label}</p>
                <p class="text-xs text-muted-foreground truncate leading-tight mt-0.5">{detail}</p>
            </div>
        </div>
    {/if}
{/snippet}

<header class="flex h-16 shrink-0 items-center gap-3 px-4 border-b">
    <Sidebar.Trigger class="-ms-1" />
    <Button variant="ghost" size="icon-sm" onclick={() => goto('/nodes')} aria-label="Back to nodes">
        <ArrowLeftIcon class="size-4" />
    </Button>
    <span class="text-sm font-medium">Nodes</span>
</header>

<div class="flex-1 overflow-y-auto">
    {#if loading}
        <p class="px-6 py-8 text-sm text-muted-foreground">Loading...</p>
    {:else if error}
        <p class="px-6 py-8 text-sm text-destructive">{error}</p>
    {:else if node}
        <div class="px-6 pt-5 pb-0">
            <div class="flex items-start gap-3 mb-4">
                <div class="flex items-center justify-center w-10 h-10 rounded-lg border bg-muted shrink-0">
                    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="2" y="3" width="20" height="14" rx="2"/>
                        <path d="M8 21h8M12 17v4"/>
                    </svg>
                </div>
                <div class="flex flex-col gap-0.5 min-w-0">
                    <h1 class="text-base font-semibold leading-tight">{node.hostname}</h1>
                    <span class="text-xs text-muted-foreground">{node.ip_address ?? 'no ip'}</span>
                </div>
                <div class="ml-auto flex items-center gap-1.5 shrink-0">
                    {#if node.cordoned}
                        <span class="text-xs px-2 py-0.5 rounded-full font-medium bg-yellow-500/15 text-yellow-600">cordoned</span>
                    {/if}
                    <span class="inline-block w-2 h-2 rounded-full {statusDotClass(node.status)}"></span>
                    <span class="text-xs font-medium">{node.status.toLowerCase()}</span>
                </div>
            </div>

            <div class="flex flex-wrap gap-1 pb-0">
                <Button variant="outline" size="sm" class="text-xs h-7 shrink-0 gap-1.5" disabled>
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="3"/>
                        <path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M16.95 16.95l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M16.95 7.05l2.83-2.83"/>
                    </svg>
                    BMC / iDRAC
                </Button>
                <Button variant="outline" size="sm" class="text-xs h-7 shrink-0 gap-1.5" onclick={handleReboot} disabled={powerActionBusy}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0 3 3L22 7l-3-3m-3.5 3.5L19 4"/>
                    </svg>
                    Reboot
                </Button>
                {#if node.cordoned}
                    <Button variant="outline" size="sm" class="text-xs h-7 shrink-0 gap-1.5" onclick={handleUncordon} disabled={powerActionBusy}>
                        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M5 12h14M12 5l7 7-7 7"/>
                        </svg>
                        Uncordon
                    </Button>
                {:else}
                    <Button variant="outline" size="sm" class="text-xs h-7 shrink-0 gap-1.5" onclick={handleDrain} disabled={powerActionBusy}>
                        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M5 12h14M12 5l7 7-7 7"/>
                        </svg>
                        Drain
                    </Button>
                {/if}
                <Button variant="outline" size="sm" class="text-xs h-7 shrink-0 gap-1.5 text-red-500 hover:text-red-500" onclick={handlePowerOff} disabled={powerActionBusy}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M18.36 6.64a9 9 0 1 1-12.73 0"/><line x1="12" y1="2" x2="12" y2="12"/>
                    </svg>
                    Power off
                </Button>
            </div>

            {#if powerActionError}
                <p class="text-xs text-destructive mt-1.5">{powerActionError}</p>
            {/if}

            <div class="flex gap-0 mt-3 border-b">
                {#each tabs as tab}
                    <button
                        class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {activeTab === tab.id ? 'border-foreground text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}"
                        onclick={() => activeTab = tab.id}
                    >
                        {tab.label}
                    </button>
                {/each}
            </div>
        </div>

        <div>
            {#if activeTab === 'summary'}
                <div class="flex flex-col gap-0">
                    <div class="px-6 py-4">
                        <div class="flex items-center justify-between mb-3">
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Live Load</span>
                            {#if metricsLoading}
                                <span class="text-xs text-muted-foreground">Loading...</span>
                            {:else if metricsLive}
                                <span class="text-xs text-emerald-500 px-2 py-0.5 border border-emerald-500/30 rounded-md flex items-center gap-1.5">
                                    <span class="inline-block w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
                                    live
                                </span>
                            {:else if metrics?.timestamp}
                                <span class="text-xs text-muted-foreground px-2 py-0.5 border rounded-md">
                                    {refreshedLabel(metrics.timestamp)}
                                </span>
                            {/if}
                        </div>

                        {#if metricsError}
                            <p class="text-sm text-muted-foreground py-4">{metricsError}</p>
                        {:else}
                            <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                                {#if true}
                                    {@const cpuVal = clampPct(metrics?.cpu_usage_percent ?? null)}
                                    {@const cpuDetail = metrics?.cpu_model ?? ((metrics?.cpu_cores ?? '-') + ' cores')}
                                    {@render gaugeCard(cpuVal, 'CPU', cpuDetail)}
                                {/if}
                                {#if true}
                                    {@const memVal = clampPct(memPct())}
                                    {@const memDetail = bytesToGb(metrics?.memory_total_bytes ?? null)}
                                    {@render gaugeCard(memVal, 'Memory', memDetail)}
                                {/if}
                                {#if true}
                                    {@const diskVal = clampPct(diskPct())}
                                    {@const diskDetail = bytesToGb(metrics?.disk_total_bytes ?? null)}
                                    {@render gaugeCard(diskVal, 'Disk', diskDetail)}
                                {/if}
                                {#if true}
                                    {@const rxPct = netRxPct()}
                                    {@const txPct = netTxPct()}
                                    {@const outerR = 20}
                                    {@const innerR = 13}
                                    {@const outerC = 2 * Math.PI * outerR}
                                    {@const innerC = 2 * Math.PI * innerR}
                                    <div class="border rounded-lg p-3 flex items-center gap-3">
                                        <div class="relative shrink-0">
                                            <svg width="52" height="52" viewBox="0 0 52 52">
                                                <circle cx="26" cy="26" r={outerR} fill="none" stroke="#3b82f6" stroke-width="4" stroke-opacity="0.12"/>
                                                <circle
                                                    cx="26" cy="26" r={outerR} fill="none"
                                                    stroke="#3b82f6" stroke-width="4"
                                                    stroke-dasharray="{((rxPct / 100) * outerC).toFixed(1)} {outerC.toFixed(1)}"
                                                    stroke-linecap="round"
                                                    transform="rotate(-180 26 26)"
                                                />
                                                <circle cx="26" cy="26" r={innerR} fill="none" stroke="#a855f7" stroke-width="4" stroke-opacity="0.12"/>
                                                <circle
                                                    cx="26" cy="26" r={innerR} fill="none"
                                                    stroke="#a855f7" stroke-width="4"
                                                    stroke-dasharray="{((txPct / 100) * innerC).toFixed(1)} {innerC.toFixed(1)}"
                                                    stroke-linecap="round"
                                                    transform="rotate(-180 26 26)"
                                                />
                                            </svg>
                                        </div>
                                        <div class="min-w-0">
                                            <p class="text-xs font-medium">Network</p>
                                            <p class="text-xs leading-tight mt-0.5" style="color: #3b82f6">
                                                <span class="font-medium">RX</span> {formatBytes(metrics?.network_rx_bytes ?? null)}
                                            </p>
                                            <p class="text-xs leading-tight" style="color: #a855f7">
                                                <span class="font-medium">TX</span> {formatBytes(metrics?.network_tx_bytes ?? null)}
                                            </p>
                                        </div>
                                    </div>
                                {/if}
                            </div>
                        {/if}
                    </div>

                    <div class="border-t mx-6"></div>

                    <div class="px-6 py-4">
                        <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Host & Lifecycle</span>
                        <div class="mt-3 border rounded-lg divide-y text-sm">
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">Hostname</span>
                                <span class="font-medium">{node.hostname}</span>
                            </div>
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">IP address</span>
                                <span class="font-mono text-xs">{node.ip_address ?? '-'}</span>
                            </div>
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">OS</span>
                                <span>{node.os_type} {node.os_version}</span>
                            </div>
                            {#if metrics?.kernel_version}
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Kernel</span>
                                    <span class="font-mono text-xs">{metrics.kernel_version}</span>
                                </div>
                            {/if}
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">Architecture</span>
                                <span>{node.architecture}</span>
                            </div>
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">Agent version</span>
                                <span class="font-mono text-xs">{node.agent_version}</span>
                            </div>
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">Uptime</span>
                                <span>{formatUptime(metrics?.uptime_seconds ?? null)}</span>
                            </div>
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">Registered</span>
                                <span class="text-xs">{node.registered_at.slice(0, 16)}</span>
                            </div>
                            <div class="flex justify-between px-3 py-2">
                                <span class="text-muted-foreground">Last heartbeat</span>
                                <span class="text-xs">{node.last_heartbeat ? node.last_heartbeat.slice(0, 16) : 'never'}</span>
                            </div>
                        </div>
                    </div>
                </div>

            {:else if activeTab === 'hardware'}
                <div class="px-6 py-4 flex flex-col gap-4">
                    {#if metricsLoading}
                        <p class="text-sm text-muted-foreground">Loading...</p>
                    {:else if metrics}
                        <div>
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">CPU</span>
                            <div class="mt-2 border rounded-lg divide-y text-sm">
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Model</span>
                                    <span class="text-xs text-right max-w-[60%]">{metrics.cpu_model ?? '-'}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Cores</span>
                                    <span>{metrics.cpu_cores ?? '-'}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Threads</span>
                                    <span>{metrics.cpu_threads ?? '-'}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Usage</span>
                                    <span>{pctStr(metrics.cpu_usage_percent)}</span>
                                </div>
                            </div>
                        </div>
                        <div>
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Memory</span>
                            <div class="mt-2 border rounded-lg divide-y text-sm">
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Total</span>
                                    <span>{bytesToGb(metrics.memory_total_bytes)}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Used</span>
                                    <span>{bytesToGb(metrics.memory_used_bytes)}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Usage</span>
                                    <span>{pctStr(memPct())}</span>
                                </div>
                            </div>
                        </div>
                        <div>
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Storage</span>
                            <div class="mt-2 border rounded-lg divide-y text-sm">
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Total</span>
                                    <span>{bytesToGb(metrics.disk_total_bytes)}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Used</span>
                                    <span>{bytesToGb(metrics.disk_used_bytes)}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Usage</span>
                                    <span>{pctStr(diskPct())}</span>
                                </div>
                            </div>
                        </div>
                    {:else}
                        <p class="text-sm text-muted-foreground">{metricsError ?? 'No data'}</p>
                    {/if}
                </div>

            {:else if activeTab === 'workloads'}
                <div class="px-6 py-4">
                    <p class="text-sm text-muted-foreground">Workload scheduling coming soon.</p>
                </div>

            {:else if activeTab === 'network'}
                <div class="px-6 py-4 flex flex-col gap-4">
                    {#if metrics}
                        <div>
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Interface</span>
                            <div class="mt-2 border rounded-lg divide-y text-sm">
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">IP address</span>
                                    <span class="font-mono text-xs">{node?.ip_address ?? '-'}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">RX</span>
                                    <span>{metrics.network_rx_bytes != null ? (metrics.network_rx_bytes / 1_048_576).toFixed(2) + ' MB/s' : '-'}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">TX</span>
                                    <span>{metrics.network_tx_bytes != null ? (metrics.network_tx_bytes / 1_048_576).toFixed(2) + ' MB/s' : '-'}</span>
                                </div>
                            </div>
                        </div>
                    {:else}
                        <p class="text-sm text-muted-foreground">{metricsError ?? 'No data'}</p>
                    {/if}
                </div>

            {:else if activeTab === 'tasks'}
                <div class="px-6 py-4">
                    <p class="text-sm text-muted-foreground">No active tasks.</p>
                </div>
            {/if}
        </div>
    {/if}
</div>
