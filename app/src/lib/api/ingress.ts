import { authedFetch } from './http';

const API_BASE = '/api';

export interface IngressRoute {
    id: string;
    resource_group_id: string;
    workload_id: string;
    gateway_id: string | null;
    hostname: string;
    container_port: number;
    tls: boolean;
    created_at: string;
}

export interface CreateIngressRequest {
    hostname: string;
    gateway_id: string;
    workload_id: string;
    container_port: number;
    tls: boolean;
}

async function failure(res: Response, fallback: string): Promise<Error> {
    const body = await res.json().catch(() => ({ error: `${fallback}: ${res.status}` }));
    return new Error(body.error ?? `${fallback}: ${res.status}`);
}

export async function listIngress(token: string, groupId: string): Promise<IngressRoute[]> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/ingress`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to list ingress routes');
    return res.json();
}

export async function createIngress(token: string, groupId: string, request: CreateIngressRequest): Promise<IngressRoute> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/ingress`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
    });
    if (!res.ok) throw await failure(res, 'Failed to create ingress route');
    return res.json();
}

export async function deleteIngress(token: string, groupId: string, routeId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/ingress/${routeId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to delete ingress route');
}

export interface Gateway {
    id: string;
    resource_group_id: string;
    workload_id: string;
    name: string;
    http_port: number;
    https_port: number;
    endpoint: string;
    status: string | null;
    created_at: string;
}

export interface CreateGatewayRequest {
    name: string;
    http_port: number;
    https_port: number;
}

export interface DnsRecord {
    id: string;
    resource_group_id: string;
    name: string;
    record_type: 'A' | 'CNAME';
    value: string;
    ttl: number;
    created_at: string;
}

export interface CreateDnsRecordRequest {
    name: string;
    record_type: 'A' | 'CNAME';
    value: string;
    ttl?: number;
}

export interface DnsZone {
    zone: string;
    servers: string[];
}

export async function listGateways(token: string, groupId: string): Promise<Gateway[]> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/gateways`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to list gateways');
    return res.json();
}

export async function createGateway(token: string, groupId: string, request: CreateGatewayRequest): Promise<Gateway> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/gateways`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
    });
    if (!res.ok) throw await failure(res, 'Failed to create gateway');
    return res.json();
}

export async function deleteGateway(token: string, groupId: string, gatewayId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/gateways/${gatewayId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to delete gateway');
}

export async function listDnsRecords(token: string, groupId: string): Promise<DnsRecord[]> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/dns-records`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to list dns records');
    return res.json();
}

export async function createDnsRecord(token: string, groupId: string, request: CreateDnsRecordRequest): Promise<DnsRecord> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/dns-records`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
    });
    if (!res.ok) throw await failure(res, 'Failed to create dns record');
    return res.json();
}

export async function deleteDnsRecord(token: string, groupId: string, recordId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/resource-groups/${groupId}/dns-records/${recordId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to delete dns record');
}

export async function getDnsZone(token: string): Promise<DnsZone> {
    const res = await authedFetch(`${API_BASE}/dns/zone`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to load dns zone');
    return res.json();
}
