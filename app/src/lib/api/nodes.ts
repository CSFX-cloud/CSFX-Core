import { api, openSocketUrl } from './http';

export interface Node {
    id: string;
    name: string;
    hostname: string;
    ip_address: string | null;
    os_type: string;
    os_version: string;
    architecture: string;
    agent_version: string;
    status: string;
    last_heartbeat: string | null;
    registered_at: string;
    cordoned: boolean;
    maintenance_until: string | null;
    role: 'master' | 'worker';
    cpu_overcommit_ratio: number;
    memory_overcommit_ratio: number;
    disk_overcommit_ratio: number;
}

export interface NodeMetrics {
    agent_id: string;
    hostname: string;
    status: string;
    cpu_usage_percent: number | null;
    memory_total_bytes: number | null;
    memory_used_bytes: number | null;
    disk_total_bytes: number | null;
    disk_used_bytes: number | null;
}

export interface NodeMetricsLatest {
    id: string;
    agent_id: string;
    timestamp: string;
    cpu_model: string | null;
    cpu_cores: number | null;
    cpu_threads: number | null;
    cpu_usage_percent: number | null;
    memory_total_bytes: number | null;
    memory_used_bytes: number | null;
    memory_usage_percent: number | null;
    disk_total_bytes: number | null;
    disk_used_bytes: number | null;
    disk_usage_percent: number | null;
    network_rx_bytes: number | null;
    network_tx_bytes: number | null;
    os_name: string | null;
    os_version: string | null;
    kernel_version: string | null;
    hostname: string | null;
    uptime_seconds: number | null;
}

export interface ClusterStats {
    node_count: number;
    online_count: number;
    total_cpu_cores: number;
    avg_cpu_usage_percent: number;
    total_memory_bytes: number;
    used_memory_bytes: number;
    total_disk_bytes: number;
    used_disk_bytes: number;
    nodes: NodeMetrics[];
}

export const listNodes = () =>
    api<Node[]>('GET', '/agents');

export const getClusterStats = () =>
    api<ClusterStats>('GET', '/system/stats');

export const getNode = (id: string) =>
    api<Node>('GET', `/agents/${id}`);

export const getNodeMetricsLatest = (id: string) =>
    api<NodeMetricsLatest>('GET', `/agents/${id}/metrics/latest`);

export interface LiveNodeMetrics {
    cpu_usage_percent: number;
    cpu_cores: number;
    memory_total_bytes: number;
    memory_used_bytes: number;
    disk_total_bytes: number;
    disk_used_bytes: number;
    network_rx_bytes: number;
    network_tx_bytes: number;
    uptime_seconds: number;
}

export async function openNodeMetricsSocket(agentId: string): Promise<WebSocket> {
    return new WebSocket(await openSocketUrl(`/agents/${agentId}/metrics/ticket`, `/agents/${agentId}/metrics/stream`));
}

export interface SmartInfo {
    health: string;
    power_on_hours: number | null;
    temperature_celsius: number | null;
    reallocated_sectors: number | null;
}

export interface DiskInfo {
    device: string;
    model: string | null;
    media_type: string;
    total_bytes: number;
    used_bytes: number;
    mount_point: string | null;
    smart: SmartInfo | null;
}

export const getNodeDisks = (id: string) =>
    api<DiskInfo[]>('GET', `/agents/${id}/disks`);

export interface HealthHistoryPoint {
    bucket: string;
    online_count: number;
}

export const getHealthHistory = (range: '1h' | '24h' | '7d' | '30d') =>
    api<HealthHistoryPoint[]>('GET', `/system/stats/history?range=${range}`);

export interface UsageHistoryPoint {
    bucket: string;
    cpu: number;
    memory: number;
    disk: number;
}

export type UsageRange = '1h' | '24h' | '7d';

export function getUsageHistory(range: UsageRange, agentId?: string): Promise<UsageHistoryPoint[]> {
    const agentQuery = agentId ? `&agent_id=${encodeURIComponent(agentId)}` : '';
    return api<UsageHistoryPoint[]>('GET', `/system/stats/usage?range=${range}${agentQuery}`);
}

export const rebootNode = (id: string) =>
    api<void>('POST', `/agents/${id}/power`, { action: 'reboot' });

export const powerOffNode = (id: string) =>
    api<void>('POST', `/agents/${id}/power`, { action: 'poweroff' });

export const drainNode = (id: string) =>
    api<void>('POST', `/agents/${id}/drain`);

export const uncordonNode = (id: string) =>
    api<void>('POST', `/agents/${id}/uncordon`);

export const cordonNode = (id: string) =>
    api<void>('POST', `/agents/${id}/cordon`);

export const removeNode = (id: string) =>
    api<void>('DELETE', `/cluster/nodes/${id}`);

export interface JoinToken {
    join_string: string;
    role: 'master' | 'worker';
    expires_at: string;
}

export const createJoinToken = (role: JoinToken['role'], ttlHours: number) =>
    api<JoinToken>('POST', '/cluster/join-tokens', { role, ttl_hours: ttlHours });
