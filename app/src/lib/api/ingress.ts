import { api } from './http';

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

export const listIngress = (groupId: string) =>
    api<IngressRoute[]>('GET', `/resource-groups/${groupId}/ingress`);

export const createIngress = (groupId: string, request: CreateIngressRequest) =>
    api<IngressRoute>('POST', `/resource-groups/${groupId}/ingress`, request);

export const deleteIngress = (groupId: string, routeId: string) =>
    api<void>('DELETE', `/resource-groups/${groupId}/ingress/${routeId}`);

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
    console_host: string;
}

export const listGateways = (groupId: string) =>
    api<Gateway[]>('GET', `/resource-groups/${groupId}/gateways`);

export const createGateway = (groupId: string, request: CreateGatewayRequest) =>
    api<Gateway>('POST', `/resource-groups/${groupId}/gateways`, request);

export const deleteGateway = (groupId: string, gatewayId: string) =>
    api<void>('DELETE', `/resource-groups/${groupId}/gateways/${gatewayId}`);

export const listDnsRecords = (groupId: string) =>
    api<DnsRecord[]>('GET', `/resource-groups/${groupId}/dns-records`);

export const createDnsRecord = (groupId: string, request: CreateDnsRecordRequest) =>
    api<DnsRecord>('POST', `/resource-groups/${groupId}/dns-records`, request);

export const deleteDnsRecord = (groupId: string, recordId: string) =>
    api<void>('DELETE', `/resource-groups/${groupId}/dns-records/${recordId}`);

export const getDnsZone = () =>
    api<DnsZone>('GET', '/dns/zone');
