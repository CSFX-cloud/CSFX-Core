<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { auth } from '$lib/auth/store.svelte';
    import { getNode, getNodeMetricsLatest, openNodeMetricsSocket, rebootNode, powerOffNode, drainNode, uncordonNode, getNodeDisks, type LiveNodeMetrics, type Node, type NodeMetricsLatest, type DiskInfo } from '$lib/api/nodes';
    import { listEvents, setMaintenance, clearMaintenance, type AlertEvent } from '$lib/api/events';
    import { listWorkloads, type Workload } from '$lib/api/resource-groups';
    import { Button } from '$lib/components/ui/button/index.js';
    import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
    import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
    import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
    import PowerIcon from '@lucide/svelte/icons/power';
    import GlobeIcon from '@lucide/svelte/icons/globe';
    import TagIcon from '@lucide/svelte/icons/tag';
    import WrenchIcon from '@lucide/svelte/icons/wrench';
    import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
    import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
    import StatusBadge from '$lib/components/status-badge.svelte';
    import BoxIcon from '@lucide/svelte/icons/box';

    const nodeId: string = $page.params.id;

    let node = $state<Node | null>(null);
    let loading = $state(true);
    let error = $state<string | null>(null);

    let metrics = $state<NodeMetricsLatest | null>(null);
    let metricsError = $state<string | null>(null);
    let metricsLoading = $state(false);
    let metricsLive = $state(false);
    let activeTab = $state<'summary' | 'hardware' | 'storage' | 'workloads' | 'network' | 'alerts' | 'tasks'>('summary');
    let liveSocket: WebSocket | null = null;
    let powerActionBusy = $state(false);
    let powerActionError = $state<string | null>(null);

    const HISTORY_LENGTH = 60;
    type HistoryPoint = { cpu: number; memory: number; rxBps: number; txBps: number };
    let history = $state<HistoryPoint[]>([]);
    let lastNetSample: { rx: number; tx: number; at: number } | null = null;

    let alerts = $state<AlertEvent[]>([]);
    let alertsLoading = $state(true);
    let alertsError = $state<string | null>(null);
    let openAlerts = $derived(alerts.filter((a) => a.status === 'open'));

    let workloads = $state<Workload[]>([]);
    let workloadsLoading = $state(true);
    let workloadsError = $state<string | null>(null);

    let disks = $state<DiskInfo[]>([]);
    let disksLoading = $state(false);
    let disksError = $state<string | null>(null);
    let disksLoaded = false;

    let maintenanceDialog = $state<HTMLDialogElement | null>(null);
    let maintenanceMinutes = $state('60');
    let maintenanceBusy = $state(false);
    let maintenanceError = $state<string | null>(null);
    let nowTick = $state(Date.now());
    let isInMaintenance = $derived(
        node?.maintenance_until != null && new Date(node.maintenance_until).getTime() > nowTick,
    );
    let maintenanceRemaining = $derived.by(() => {
        if (!node?.maintenance_until) return null;
        const diffMs = new Date(node.maintenance_until).getTime() - nowTick;
        if (diffMs <= 0) return null;
        const totalMinutes = Math.ceil(diffMs / 60000);
        const hours = Math.floor(totalMinutes / 60);
        const minutes = totalMinutes % 60;
        if (hours > 0) return `${hours}h ${minutes}m left`;
        return `${minutes}m left`;
    });

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

    async function loadAlerts() {
        if (!auth.token) return;
        alertsLoading = true;
        try {
            alerts = await listEvents(auth.token, { agentId: nodeId });
        } catch (e) {
            alertsError = e instanceof Error ? e.message : 'Failed to load alerts';
        } finally {
            alertsLoading = false;
        }
    }

    async function loadWorkloads() {
        if (!auth.token) return;
        workloadsLoading = true;
        try {
            const all = await listWorkloads(auth.token);
            workloads = all.filter((w) => w.assigned_agent_id === nodeId);
        } catch (e) {
            workloadsError = e instanceof Error ? e.message : 'Failed to load workloads';
        } finally {
            workloadsLoading = false;
        }
    }

    async function loadDisks() {
        if (!auth.token || disksLoaded) return;
        disksLoading = true;
        try {
            disks = await getNodeDisks(auth.token, nodeId);
            disksLoaded = true;
        } catch (e) {
            disksError = e instanceof Error ? e.message : 'Failed to load disks';
        } finally {
            disksLoading = false;
        }
    }

    $effect(() => {
        if (activeTab === 'storage') loadDisks();
    });

    async function handleSetMaintenance() {
        if (!auth.token || !node) return;
        const minutes = parseInt(maintenanceMinutes);
        if (!minutes || minutes < 1) return;
        maintenanceBusy = true;
        maintenanceError = null;
        try {
            await setMaintenance(auth.token, node.id, minutes);
            node = await getNode(auth.token, nodeId);
            maintenanceDialog?.close();
        } catch (e) {
            maintenanceError = e instanceof Error ? e.message : 'Failed to set maintenance';
        } finally {
            maintenanceBusy = false;
        }
    }

    async function handleClearMaintenance() {
        if (!auth.token || !node) return;
        maintenanceBusy = true;
        maintenanceError = null;
        try {
            await clearMaintenance(auth.token, node.id);
            node = await getNode(auth.token, nodeId);
        } catch (e) {
            maintenanceError = e instanceof Error ? e.message : 'Failed to clear maintenance';
        } finally {
            maintenanceBusy = false;
        }
    }

    let loadStarted = false;

    let pollInterval: ReturnType<typeof setInterval> | null = null;
    let tickInterval: ReturnType<typeof setInterval> | null = null;

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            loadNode().then(() => {
                loadMetrics(nodeId).then(() => openLiveMetrics(nodeId));
            });
            loadAlerts();
            loadWorkloads();
            pollInterval = setInterval(() => {
                if (!metricsLive) loadMetrics(nodeId);
            }, 5000);
            tickInterval = setInterval(() => {
                nowTick = Date.now();
            }, 30000);
        }

        return () => {
            closeLiveMetrics();
            if (pollInterval) clearInterval(pollInterval);
            if (tickInterval) clearInterval(tickInterval);
        };
    });

    async function loadMetrics(id: string) {
        if (!auth.token) return;
        metricsLoading = true;
        try {
            metrics = await getNodeMetricsLatest(auth.token, id);
            pushHistorySample(
                metrics.cpu_usage_percent,
                metrics.memory_used_bytes,
                metrics.memory_total_bytes,
                metrics.network_rx_bytes,
                metrics.network_tx_bytes,
            );
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
        pushHistoryPoint(sample);
    }

    function pushHistoryPoint(sample: LiveNodeMetrics) {
        pushHistorySample(sample.cpu_usage_percent, sample.memory_used_bytes, sample.memory_total_bytes, sample.network_rx_bytes, sample.network_tx_bytes);
    }

    function pushHistorySample(
        cpuPercent: number | null,
        memoryUsedBytes: number | null,
        memoryTotalBytes: number | null,
        networkRxBytes: number | null,
        networkTxBytes: number | null,
    ) {
        const now = Date.now();
        let rxBps = 0;
        let txBps = 0;
        if (lastNetSample && networkRxBytes != null && networkTxBytes != null) {
            const elapsed = (now - lastNetSample.at) / 1000;
            if (elapsed > 0) {
                rxBps = Math.max(0, (networkRxBytes - lastNetSample.rx) / elapsed);
                txBps = Math.max(0, (networkTxBytes - lastNetSample.tx) / elapsed);
            }
        }
        if (networkRxBytes != null && networkTxBytes != null) {
            lastNetSample = { rx: networkRxBytes, tx: networkTxBytes, at: now };
        }

        const memPercent = memoryTotalBytes != null && memoryTotalBytes > 0 && memoryUsedBytes != null
            ? (memoryUsedBytes / memoryTotalBytes) * 100
            : 0;

        const next = [...history, { cpu: cpuPercent ?? 0, memory: memPercent, rxBps, txBps }];
        history = next.length > HISTORY_LENGTH ? next.slice(next.length - HISTORY_LENGTH) : next;
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
        const pb = 1_125_899_906_842_624;
        const tb = 1_099_511_627_776;
        const gb = 1_073_741_824;
        if (bytes >= pb) return (bytes / pb).toFixed(1) + ' PB';
        if (bytes >= tb) return (bytes / tb).toFixed(1) + ' TB';
        return (bytes / gb).toFixed(1) + ' GB';
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

    const GAUGE_ARC_FRACTION = 0.7;
    const GAUGE_ROTATION_DEG = 90 + (360 * (1 - GAUGE_ARC_FRACTION)) / 2;

    function gaugeArc(value: number, radius: number): { dasharray: string; trackDasharray: string; circumference: number } {
        const circumference = 2 * Math.PI * radius;
        const arcLength = circumference * GAUGE_ARC_FRACTION;
        const filled = (value / 100) * arcLength;
        return {
            dasharray: `${filled.toFixed(1)} ${circumference.toFixed(1)}`,
            trackDasharray: `${arcLength.toFixed(1)} ${circumference.toFixed(1)}`,
            circumference,
        };
    }

    function gaugeColor(value: number): string {
        if (value > 80) return '#ef4444';
        if (value > 60) return '#eab308';
        return 'currentColor';
    }

    function formatBandwidth(bytesPerSecond: number): string {
        if (bytesPerSecond >= 1_073_741_824) return (bytesPerSecond / 1_073_741_824).toFixed(1) + ' GB/s';
        if (bytesPerSecond >= 1_048_576) return (bytesPerSecond / 1_048_576).toFixed(1) + ' MB/s';
        if (bytesPerSecond >= 1024) return (bytesPerSecond / 1024).toFixed(1) + ' KB/s';
        return bytesPerSecond.toFixed(0) + ' B/s';
    }

    function sparklineXRange(values: number[], width: number): { firstX: number; lastX: number } {
        const step = width / (HISTORY_LENGTH - 1);
        const offset = HISTORY_LENGTH - values.length;
        return { firstX: offset * step, lastX: (HISTORY_LENGTH - 1) * step };
    }

    function sparklinePath(values: number[], width: number, height: number, max: number): string {
        if (values.length < 2) return '';
        const step = width / (HISTORY_LENGTH - 1);
        const offset = HISTORY_LENGTH - values.length;
        const points = values.map((v, i) => {
            const x = (offset + i) * step;
            const y = height - (Math.min(v, max) / max) * height;
            return `${x.toFixed(1)},${y.toFixed(1)}`;
        });
        return `M${points.join(' L')}`;
    }

    function sparklineFillPath(linePath: string, values: number[], width: number, height: number): string {
        if (!linePath) return '';
        const { firstX, lastX } = sparklineXRange(values, width);
        return `${linePath} L${lastX.toFixed(1)},${height} L${firstX.toFixed(1)},${height} Z`;
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
        { id: 'storage', label: 'Storage' },
        { id: 'workloads', label: 'Workloads' },
        { id: 'network', label: 'Network' },
        { id: 'alerts', label: 'Alerts' },
        { id: 'tasks', label: 'Tasks' },
    ];
</script>

{#snippet gaugeCard(value: number, label: string, detail: string)}
    {#if true}
        {@const arc = gaugeArc(value, 44)}
        <div class="p-3 flex flex-col items-center">
            <div class="relative shrink-0">
                <svg width="112" height="112" viewBox="0 0 112 112" style="color: {gaugeColor(value)}">
                    <circle
                        cx="56" cy="56" r="44" fill="none"
                        stroke="currentColor" stroke-width="8" stroke-opacity="0.12"
                        stroke-dasharray={arc.trackDasharray}
                        stroke-linecap="round"
                        transform="rotate({GAUGE_ROTATION_DEG} 56 56)"
                    />
                    <circle
                        cx="56" cy="56" r="44" fill="none"
                        stroke="currentColor" stroke-width="8"
                        stroke-dasharray={arc.dasharray}
                        stroke-linecap="round"
                        transform="rotate({GAUGE_ROTATION_DEG} 56 56)"
                    />
                </svg>
                <div class="absolute inset-0 flex items-center justify-center">
                    <span class="text-xl font-semibold leading-none">{value.toFixed(0)}%</span>
                </div>
            </div>
            <div class="flex flex-col items-center gap-0.5 mt-1 text-center">
                <span class="text-xs font-medium leading-tight">{label}</span>
                <span class="text-[11px] text-muted-foreground truncate leading-tight max-w-full">{detail}</span>
            </div>
        </div>
    {/if}
{/snippet}

{#snippet historyChart(label: string, currentValue: number, currentLabel: string, path: string, values: number[], color: string, maxValue: number)}
    {@const fillPath = sparklineFillPath(path, values, 440, 120)}
    {@const gradientId = 'fill-' + label.toLowerCase().replace(/[^a-z0-9]+/g, '-')}
    <div class="p-3">
        <div class="flex items-center justify-between mb-2">
            <p class="text-xs font-medium">{label}</p>
            <p class="text-sm font-semibold tabular-nums" style="color: {color}">{currentLabel}</p>
        </div>
        <div class="flex gap-2">
            <svg viewBox="0 0 440 120" width="100%" height="120" preserveAspectRatio="none" class="flex-1 min-w-0">
                <defs>
                    <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stop-color={color} stop-opacity="0.25"/>
                        <stop offset="100%" stop-color={color} stop-opacity="0"/>
                    </linearGradient>
                </defs>
                <path d={fillPath} fill="url(#{gradientId})" stroke="none"/>
                <path d={path} fill="none" stroke={color} stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
            </svg>
            <div class="flex flex-col justify-between text-[11px] text-muted-foreground shrink-0 py-0.5">
                <span>{maxValue.toFixed(0)}%</span>
                <span>0%</span>
            </div>
        </div>
    </div>
{/snippet}

{#snippet networkHistoryChart(points: { rxBps: number; txBps: number }[], max: number)}
    {@const rxValues = points.map((p) => p.rxBps)}
    {@const txValues = points.map((p) => p.txBps)}
    {@const rxPath = sparklinePath(rxValues, 440, 120, max)}
    {@const txPath = sparklinePath(txValues, 440, 120, max)}
    {@const rxFillPath = sparklineFillPath(rxPath, rxValues, 440, 120)}
    {@const lastRx = points[points.length - 1]?.rxBps ?? 0}
    {@const lastTx = points[points.length - 1]?.txBps ?? 0}
    <div class="p-3">
        <div class="flex items-center justify-between mb-2">
            <p class="text-xs font-medium">Network</p>
            <p class="text-sm font-medium tabular-nums">
                <span style="color: #3b82f6">RX {formatBandwidth(lastRx)}</span>
                <span class="text-muted-foreground mx-1">/</span>
                <span style="color: #a855f7">TX {formatBandwidth(lastTx)}</span>
            </p>
        </div>
        <div class="flex gap-2">
            <svg viewBox="0 0 440 120" width="100%" height="120" preserveAspectRatio="none" class="flex-1 min-w-0">
                <defs>
                    <linearGradient id="fill-net-rx" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stop-color="#3b82f6" stop-opacity="0.2"/>
                        <stop offset="100%" stop-color="#3b82f6" stop-opacity="0"/>
                    </linearGradient>
                </defs>
                <path d={rxFillPath} fill="url(#fill-net-rx)" stroke="none"/>
                <path d={rxPath} fill="none" stroke="#3b82f6" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
                <path d={txPath} fill="none" stroke="#a855f7" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
            </svg>
            <div class="flex flex-col justify-between text-[11px] text-muted-foreground shrink-0 py-0.5">
                <span>{formatBandwidth(max)}</span>
                <span>0 B/s</span>
            </div>
        </div>
    </div>
{/snippet}

<header class="flex h-20 shrink-0 items-center gap-3 px-4">
    <Button variant="ghost" size="icon-sm" onclick={() => goto('/nodes')} aria-label="Back to nodes">
        <ArrowLeftIcon class="size-4" />
    </Button>
    {#if node}
        <div class="flex items-center justify-center w-10 h-10 rounded-lg border bg-muted shrink-0">
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="3" width="20" height="14" rx="2"/>
                <path d="M8 21h8M12 17v4"/>
            </svg>
        </div>
        <div class="flex flex-col gap-0.5 min-w-0">
            <div class="flex items-center gap-2 min-w-0">
                <span class="text-base font-semibold leading-tight truncate">{node.hostname}</span>
                <span class="inline-block w-2 h-2 rounded-full shrink-0 {statusDotClass(node.status)}" title={node.status.toLowerCase()}></span>
                {#if node.cordoned}
                    <span class="text-xs px-2 py-0.5 rounded-full font-medium bg-yellow-500/15 text-yellow-600 border border-yellow-500/20 shrink-0">cordoned</span>
                {/if}
                {#if isInMaintenance}
                    <span class="text-xs px-2 py-0.5 rounded-full font-medium bg-amber-500/15 text-amber-600 border border-amber-500/20 shrink-0">
                        maintenance{maintenanceRemaining ? ` · ${maintenanceRemaining}` : ''}
                    </span>
                {/if}
            </div>
            <div class="flex items-center gap-3 text-xs text-muted-foreground leading-tight">
                <span class="flex items-center gap-1">
                    <GlobeIcon class="size-3" />
                    {node.ip_address ?? 'no ip'}
                </span>
                <span class="flex items-center gap-1">
                    <TagIcon class="size-3" />
                    v{node.agent_version}
                </span>
            </div>
        </div>

        <div class="ml-auto flex items-center gap-0.5 shrink-0">
            {#if isInMaintenance}
                <Button variant="ghost" size="icon-sm" onclick={handleClearMaintenance} disabled={maintenanceBusy} class="text-amber-500 hover:text-amber-500" aria-label="End maintenance" title="End maintenance">
                    <WrenchIcon class="size-4" />
                </Button>
            {:else}
                <Button variant="ghost" size="icon-sm" onclick={() => { maintenanceError = null; maintenanceDialog?.showModal(); }} aria-label="Start maintenance" title="Start maintenance">
                    <WrenchIcon class="size-4" />
                </Button>
            {/if}
            <Button variant="ghost" size="icon-sm" onclick={handleReboot} disabled={powerActionBusy} aria-label="Reboot" title="Reboot">
                <RotateCwIcon class="size-4" />
            </Button>
            {#if node.cordoned}
                <Button variant="ghost" size="icon-sm" onclick={handleUncordon} disabled={powerActionBusy} aria-label="Uncordon" title="Uncordon">
                    <ArrowRightIcon class="size-4" />
                </Button>
            {:else}
                <Button variant="ghost" size="icon-sm" onclick={handleDrain} disabled={powerActionBusy} aria-label="Drain" title="Drain">
                    <ArrowRightIcon class="size-4" />
                </Button>
            {/if}
            <Button variant="ghost" size="icon-sm" onclick={handlePowerOff} disabled={powerActionBusy} class="text-red-500 hover:text-red-500" aria-label="Power off" title="Power off">
                <PowerIcon class="size-4" />
            </Button>
        </div>
    {/if}
</header>

{#if node && powerActionError}
    <p class="text-xs text-destructive px-4 pt-2">{powerActionError}</p>
{/if}

<div class="flex-1 overflow-y-auto">
    {#if loading}
        <p class="px-6 py-8 text-sm text-muted-foreground">Loading...</p>
    {:else if error}
        <p class="px-6 py-8 text-sm text-destructive">{error}</p>
    {:else if node}
        <div class="px-6 pt-4 pb-0">
            <div class="flex gap-0 border-b">
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
                                    {@const outerR = 44}
                                    {@const innerR = 33}
                                    {@const outerC = 2 * Math.PI * outerR}
                                    {@const innerC = 2 * Math.PI * innerR}
                                    {@const outerArc = outerC * GAUGE_ARC_FRACTION}
                                    {@const innerArc = innerC * GAUGE_ARC_FRACTION}
                                    <div class="p-3 flex flex-col items-center">
                                        <div class="relative shrink-0">
                                            <svg width="112" height="112" viewBox="0 0 112 112">
                                                <circle
                                                    cx="56" cy="56" r={outerR} fill="none"
                                                    stroke="#3b82f6" stroke-width="8" stroke-opacity="0.12"
                                                    stroke-dasharray="{outerArc.toFixed(1)} {outerC.toFixed(1)}"
                                                    stroke-linecap="round"
                                                    transform="rotate({GAUGE_ROTATION_DEG} 56 56)"
                                                />
                                                <circle
                                                    cx="56" cy="56" r={outerR} fill="none"
                                                    stroke="#3b82f6" stroke-width="8"
                                                    stroke-dasharray="{((rxPct / 100) * outerArc).toFixed(1)} {outerC.toFixed(1)}"
                                                    stroke-linecap="round"
                                                    transform="rotate({GAUGE_ROTATION_DEG} 56 56)"
                                                />
                                                <circle
                                                    cx="56" cy="56" r={innerR} fill="none"
                                                    stroke="#a855f7" stroke-width="8" stroke-opacity="0.12"
                                                    stroke-dasharray="{innerArc.toFixed(1)} {innerC.toFixed(1)}"
                                                    stroke-linecap="round"
                                                    transform="rotate({GAUGE_ROTATION_DEG} 56 56)"
                                                />
                                                <circle
                                                    cx="56" cy="56" r={innerR} fill="none"
                                                    stroke="#a855f7" stroke-width="8"
                                                    stroke-dasharray="{((txPct / 100) * innerArc).toFixed(1)} {innerC.toFixed(1)}"
                                                    stroke-linecap="round"
                                                    transform="rotate({GAUGE_ROTATION_DEG} 56 56)"
                                                />
                                            </svg>
                                            <div class="absolute inset-0 flex items-center justify-center">
                                                <span class="text-xs font-medium leading-tight">Network</span>
                                            </div>
                                        </div>
                                        <div class="flex flex-col items-center gap-0.5 mt-1 text-center">
                                            <span class="text-[11px] leading-tight" style="color: #3b82f6">RX {formatBytes(metrics?.network_rx_bytes ?? null)}</span>
                                            <span class="text-[11px] leading-tight" style="color: #a855f7">TX {formatBytes(metrics?.network_tx_bytes ?? null)}</span>
                                        </div>
                                    </div>
                                {/if}
                            </div>
                        {/if}
                    </div>

                    <div class="border-t mx-6"></div>

                    <div class="px-6 py-4 grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-6">
                        <div>
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">History</span>
                            <div class="mt-3 border rounded-lg overflow-hidden">
                                {#if history.length < 2}
                                    <div class="grid grid-cols-1 sm:grid-cols-2 divide-y sm:divide-y-0 sm:divide-x divide-border">
                                        {#each [0, 1] as i (i)}
                                            <div class="p-3">
                                                <div class="flex items-center justify-between mb-2">
                                                    <div class="h-3 w-20 rounded bg-muted animate-pulse"></div>
                                                    <div class="h-4 w-10 rounded bg-muted animate-pulse"></div>
                                                </div>
                                                <div class="h-[120px] rounded bg-muted animate-pulse"></div>
                                            </div>
                                        {/each}
                                    </div>
                                    <div class="border-t p-3">
                                        <div class="flex items-center justify-between mb-2">
                                            <div class="h-3 w-16 rounded bg-muted animate-pulse"></div>
                                            <div class="h-4 w-24 rounded bg-muted animate-pulse"></div>
                                        </div>
                                        <div class="h-[120px] rounded bg-muted animate-pulse"></div>
                                    </div>
                                {:else}
                                    {@const cpuValues = history.map((h) => h.cpu)}
                                    {@const memValues = history.map((h) => h.memory)}
                                    {@const cpuMax = Math.max(20, ...cpuValues)}
                                    {@const memMax = Math.max(20, ...memValues)}
                                    {@const netMax = Math.max(1_048_576, ...history.map((h) => Math.max(h.rxBps, h.txBps)))}
                                    <div class="grid grid-cols-1 sm:grid-cols-2 divide-y sm:divide-y-0 sm:divide-x divide-border">
                                        {@render historyChart('CPU usage', cpuValues[cpuValues.length - 1], pctStr(cpuValues[cpuValues.length - 1]), sparklinePath(cpuValues, 440, 120, cpuMax), cpuValues, '#3b82f6', cpuMax)}
                                        {@render historyChart('Memory usage', memValues[memValues.length - 1], pctStr(memValues[memValues.length - 1]), sparklinePath(memValues, 440, 120, memMax), memValues, '#a855f7', memMax)}
                                    </div>
                                    <div class="border-t">
                                        {@render networkHistoryChart(history, netMax)}
                                    </div>
                                {/if}
                            </div>
                        </div>

                        <div class="flex flex-col h-full">
                            <div class="flex items-center justify-between mb-3">
                                <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Alerts</span>
                                {#if openAlerts.length > 0}
                                    <button class="text-xs text-muted-foreground hover:text-foreground" onclick={() => activeTab = 'alerts'}>
                                        View all
                                    </button>
                                {/if}
                            </div>
                            <div class="border rounded-lg flex-1 min-h-0">
                                {#if alertsLoading}
                                    <div class="divide-y">
                                        {#each [0, 1, 2] as i (i)}
                                            <div class="flex items-start gap-2 px-2.5 py-1.5">
                                                <div class="size-3.5 mt-0.5 shrink-0 rounded-full bg-muted animate-pulse"></div>
                                                <div class="min-w-0 flex-1 flex flex-col gap-1">
                                                    <div class="h-3 w-3/4 rounded bg-muted animate-pulse"></div>
                                                    <div class="h-2.5 w-1/2 rounded bg-muted animate-pulse"></div>
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                {:else if openAlerts.length === 0}
                                    <div class="flex flex-col items-center justify-center gap-1.5 h-full py-8 px-3 text-center">
                                        <CircleCheckIcon class="size-5 text-muted-foreground" />
                                        <p class="text-xs text-muted-foreground">No open alerts</p>
                                    </div>
                                {:else}
                                    <div class="divide-y">
                                        {#each openAlerts.slice(0, 5) as alert (alert.id)}
                                            <div class="flex items-start gap-2 px-2.5 py-1.5">
                                                <TriangleAlertIcon class="size-3.5 mt-0.5 shrink-0 text-amber-500" />
                                                <div class="min-w-0 flex-1">
                                                    <span class="text-xs font-medium leading-tight block truncate">{alert.message ?? alert.event_type}</span>
                                                    <p class="text-[11px] text-muted-foreground leading-tight">{new Date(alert.created_at).toLocaleString()}</p>
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                {/if}
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
                            <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Host</span>
                            <div class="mt-2 border rounded-lg divide-y text-sm">
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">Hostname</span>
                                    <span class="font-medium">{node.hostname}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">OS</span>
                                    <span>{node.os_type} {node.os_version}</span>
                                </div>
                                {#if metrics.kernel_version}
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
                                    <span class="text-muted-foreground">Uptime</span>
                                    <span>{formatUptime(metrics.uptime_seconds)}</span>
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
                    {#if workloadsLoading}
                        <div class="border rounded-lg overflow-hidden w-full">
                            <table class="w-full text-sm">
                                <thead class="bg-muted/50">
                                    <tr>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Name</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Image</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Status</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">CPU</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Memory</th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y">
                                    {#each [0, 1, 2] as i (i)}
                                        <tr>
                                            <td class="px-3 py-2.5"><div class="h-3.5 w-24 rounded bg-muted animate-pulse"></div></td>
                                            <td class="px-3 py-2.5"><div class="h-3.5 w-32 rounded bg-muted animate-pulse"></div></td>
                                            <td class="px-3 py-2.5"><div class="h-4 w-16 rounded-full bg-muted animate-pulse"></div></td>
                                            <td class="px-3 py-2.5"><div class="h-3.5 w-10 rounded bg-muted animate-pulse"></div></td>
                                            <td class="px-3 py-2.5"><div class="h-3.5 w-14 rounded bg-muted animate-pulse"></div></td>
                                        </tr>
                                    {/each}
                                </tbody>
                            </table>
                        </div>
                    {:else if workloadsError}
                        <p class="text-sm text-destructive">{workloadsError}</p>
                    {:else if workloads.length === 0}
                        <div class="border rounded-lg flex flex-col items-center justify-center gap-1.5 py-10 px-3 text-center">
                            <BoxIcon class="size-5 text-muted-foreground" />
                            <p class="text-xs text-muted-foreground">No workloads scheduled on this node</p>
                        </div>
                    {:else}
                        <div class="border rounded-lg overflow-hidden w-full">
                            <table class="w-full text-sm">
                                <thead class="bg-muted/50">
                                    <tr>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Name</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Image</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Status</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">CPU</th>
                                        <th class="text-left px-3 py-2 font-medium text-muted-foreground">Memory</th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y">
                                    {#each workloads as workload (workload.id)}
                                        <tr>
                                            <td class="px-3 py-2 font-medium">{workload.name}</td>
                                            <td class="px-3 py-2 text-muted-foreground font-mono text-xs">{workload.image}</td>
                                            <td class="px-3 py-2"><StatusBadge status={workload.status} /></td>
                                            <td class="px-3 py-2 text-muted-foreground">{workload.cpu_usage_percent != null ? `${workload.cpu_usage_percent.toFixed(0)}%` : '-'}</td>
                                            <td class="px-3 py-2 text-muted-foreground">{formatBytes(workload.memory_usage_bytes)}</td>
                                        </tr>
                                    {/each}
                                </tbody>
                            </table>
                        </div>
                    {/if}
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
                                    <span>{metrics.network_rx_bytes != null ? formatBandwidth(history[history.length - 1]?.rxBps ?? 0) : '-'}</span>
                                </div>
                                <div class="flex justify-between px-3 py-2">
                                    <span class="text-muted-foreground">TX</span>
                                    <span>{metrics.network_tx_bytes != null ? formatBandwidth(history[history.length - 1]?.txBps ?? 0) : '-'}</span>
                                </div>
                            </div>
                        </div>
                    {:else}
                        <p class="text-sm text-muted-foreground">{metricsError ?? 'No data'}</p>
                    {/if}
                </div>

            {:else if activeTab === 'alerts'}
                <div class="px-6 py-4">
                    {#if alertsLoading}
                        <div class="border rounded-lg divide-y w-full">
                            {#each [0, 1, 2, 3] as i (i)}
                                <div class="flex items-start gap-2 px-2.5 py-1.5">
                                    <div class="size-3.5 mt-0.5 shrink-0 rounded-full bg-muted animate-pulse"></div>
                                    <div class="min-w-0 flex-1 flex flex-col gap-1">
                                        <div class="h-3 w-1/3 rounded bg-muted animate-pulse"></div>
                                        <div class="h-2.5 w-1/4 rounded bg-muted animate-pulse"></div>
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {:else if alertsError}
                        <p class="text-sm text-destructive">{alertsError}</p>
                    {:else if alerts.length === 0}
                        <p class="text-sm text-muted-foreground">No alerts for this node.</p>
                    {:else}
                        <div class="border rounded-lg divide-y w-full">
                            {#each alerts as alert (alert.id)}
                                <div class="flex items-start gap-2 px-2.5 py-1.5 w-full">
                                    <TriangleAlertIcon class="size-3.5 mt-0.5 shrink-0 {alert.status === 'open' ? 'text-amber-500' : 'text-muted-foreground'}" />
                                    <div class="min-w-0 flex-1">
                                        <div class="flex items-center gap-1.5">
                                            <span class="text-xs font-medium truncate">{alert.message ?? alert.event_type}</span>
                                            <span class="text-[10px] px-1.5 py-px rounded font-medium shrink-0 {alert.status === 'open' ? 'bg-amber-500/15 text-amber-600' : 'bg-muted text-muted-foreground'}">
                                                {alert.status}
                                            </span>
                                        </div>
                                        <p class="text-[11px] text-muted-foreground leading-tight">
                                            {new Date(alert.created_at).toLocaleString()}
                                            {#if alert.resolved_at}
                                                &nbsp;&middot;&nbsp;resolved {new Date(alert.resolved_at).toLocaleString()}
                                            {/if}
                                        </p>
                                    </div>
                                </div>
                            {/each}
                        </div>
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

<dialog
    bind:this={maintenanceDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-sm rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
>
    <div class="flex flex-col gap-4 p-6">
        <h2 class="text-base font-semibold">Start maintenance</h2>
        <p class="text-sm text-muted-foreground">Hardware alerts for this node are suppressed until maintenance ends.</p>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="maintenance-minutes">Duration (minutes)</label>
            <input
                id="maintenance-minutes"
                type="number"
                min="1"
                class="border rounded px-3 py-1.5 text-sm bg-background"
                bind:value={maintenanceMinutes}
            />
        </div>
        {#if maintenanceError}
            <p class="text-xs text-destructive">{maintenanceError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => maintenanceDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleSetMaintenance} disabled={maintenanceBusy}>
                {maintenanceBusy ? 'Starting...' : 'Start maintenance'}
            </Button>
        </div>
    </div>
</dialog>
