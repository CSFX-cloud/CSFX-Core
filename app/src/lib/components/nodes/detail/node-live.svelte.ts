import { getNodeMetricsLatest, openNodeMetricsSocket, type LiveNodeMetrics, type NodeMetricsLatest } from "$lib/api/nodes";

export interface HistoryPoint {
    cpu: number;
    memory: number;
    rxBps: number;
    txBps: number;
}

interface NetworkSample {
    rx: number;
    tx: number;
    at: number;
}

const HISTORY_LENGTH = 60;
const POLL_INTERVAL_MS = 5000;
const MS_PER_SECOND = 1000;
const PERCENT = 100;

function percentOf(used: number | null, total: number | null): number {
    return used != null && total != null && total > 0 ? (used / total) * PERCENT : 0;
}

export class NodeLiveMetrics {
    metrics = $state<NodeMetricsLatest | null>(null);
    history = $state<HistoryPoint[]>([]);
    error = $state<string | null>(null);
    loading = $state(false);
    live = $state(false);

    #nodeId: string;
    #socket: WebSocket | null = null;
    #timer: ReturnType<typeof setInterval> | null = null;
    #lastNetwork: NetworkSample | null = null;

    constructor(nodeId: string) {
        this.#nodeId = nodeId;
    }

    async start(token: string) {
        await this.#load(token);
        this.#timer = setInterval(() => {
            if (!this.live) this.#load(token);
        }, POLL_INTERVAL_MS);
        await this.#openSocket(token);
    }

    stop() {
        this.#socket?.close();
        this.#socket = null;
        this.live = false;
        if (this.#timer) clearInterval(this.#timer);
        this.#timer = null;
    }

    async #load(token: string) {
        this.loading = true;
        try {
            const latest = await getNodeMetricsLatest(token, this.#nodeId);
            this.metrics = latest;
            this.#push(latest.cpu_usage_percent, latest.memory_used_bytes, latest.memory_total_bytes, latest.network_rx_bytes, latest.network_tx_bytes);
            this.error = null;
        } catch {
            this.error = "No metrics available";
        } finally {
            this.loading = false;
        }
    }

    async #openSocket(token: string) {
        try {
            const socket = await openNodeMetricsSocket(token, this.#nodeId);
            this.#socket = socket;
            socket.onmessage = (event) => this.#receive(event.data);
            socket.onclose = () => (this.live = false);
            socket.onerror = () => (this.live = false);
        } catch {
            this.live = false;
        }
    }

    #receive(payload: string) {
        try {
            this.#apply(JSON.parse(payload) as LiveNodeMetrics);
            this.live = true;
        } catch {
            this.live = false;
        }
    }

    #apply(sample: LiveNodeMetrics) {
        const previous = this.metrics;
        this.metrics = {
            id: previous?.id ?? "",
            agent_id: previous?.agent_id ?? "",
            timestamp: new Date().toISOString(),
            cpu_model: previous?.cpu_model ?? null,
            cpu_cores: sample.cpu_cores,
            cpu_threads: previous?.cpu_threads ?? null,
            cpu_usage_percent: sample.cpu_usage_percent,
            memory_total_bytes: sample.memory_total_bytes,
            memory_used_bytes: sample.memory_used_bytes,
            memory_usage_percent: sample.memory_total_bytes > 0 ? percentOf(sample.memory_used_bytes, sample.memory_total_bytes) : null,
            disk_total_bytes: sample.disk_total_bytes,
            disk_used_bytes: sample.disk_used_bytes,
            disk_usage_percent: sample.disk_total_bytes > 0 ? percentOf(sample.disk_used_bytes, sample.disk_total_bytes) : null,
            network_rx_bytes: sample.network_rx_bytes,
            network_tx_bytes: sample.network_tx_bytes,
            os_name: previous?.os_name ?? null,
            os_version: previous?.os_version ?? null,
            kernel_version: previous?.kernel_version ?? null,
            hostname: previous?.hostname ?? null,
            uptime_seconds: sample.uptime_seconds,
        };
        this.#push(sample.cpu_usage_percent, sample.memory_used_bytes, sample.memory_total_bytes, sample.network_rx_bytes, sample.network_tx_bytes);
    }

    #networkRates(rxBytes: number | null, txBytes: number | null, now: number): { rxBps: number; txBps: number } {
        const previous = this.#lastNetwork;
        if (rxBytes != null && txBytes != null) this.#lastNetwork = { rx: rxBytes, tx: txBytes, at: now };
        if (!previous || rxBytes == null || txBytes == null) return { rxBps: 0, txBps: 0 };
        const elapsed = (now - previous.at) / MS_PER_SECOND;
        if (elapsed <= 0) return { rxBps: 0, txBps: 0 };
        return { rxBps: Math.max(0, (rxBytes - previous.rx) / elapsed), txBps: Math.max(0, (txBytes - previous.tx) / elapsed) };
    }

    #push(cpu: number | null, memoryUsed: number | null, memoryTotal: number | null, rxBytes: number | null, txBytes: number | null) {
        const rates = this.#networkRates(rxBytes, txBytes, Date.now());
        const next = [...this.history, { cpu: cpu ?? 0, memory: percentOf(memoryUsed, memoryTotal), ...rates }];
        this.history = next.slice(-HISTORY_LENGTH);
    }
}
