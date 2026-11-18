import { api } from './http';
import { listBuckets, listResourceGroups, listVolumes, listWorkloads } from './resource-groups';

const TARGETS_PATH = '/admin/export-targets';

export type ScopeKind = 'config' | 'full' | 'resource_group' | 'workload' | 'volume' | 'bucket';
export type ExportStatus = 'running' | 'completed' | 'failed';

export interface ExportTarget {
    name: string;
    kind: 'local' | 's3';
    location: string;
    encrypted: boolean;
    default: boolean;
}

export interface LocalDestination {
    kind: 'local';
    path: string;
}

export interface S3Destination {
    kind: 's3';
    endpoint: string;
    region: string;
    bucket: string;
    prefix: string;
    access_key_id: string;
    secret_access_key: string;
}

export type ExportTargetInput = (LocalDestination | S3Destination) & {
    passphrase: string | null;
    default: boolean;
};

export interface ExportArtifact {
    name: string;
    kind: string;
    source_id: string | null;
    bytes: number;
    sha256: string;
}

export interface ExportManifest {
    csfx_version: string;
    created_at: string;
    consistency: string;
    encryption: object | null;
    artifacts: ExportArtifact[];
}

export interface ExportRecord {
    id: string;
    scope_kind: ScopeKind;
    scope_id: string | null;
    status: ExportStatus;
    manifest: ExportManifest | null;
    error: string | null;
    created_by: string | null;
    created_at: string;
    finished_at: string | null;
}

export interface ExportScope {
    kind: ScopeKind;
    id: string | null;
}

export interface RestoreOptions {
    target: string | null;
    passphrase: string | null;
    agent_id: string | null;
}

export interface ScopeResource {
    id: string;
    name: string;
}

export const listExportTargets = () => api<ExportTarget[]>('GET', TARGETS_PATH);

export const saveExportTarget = (name: string, input: ExportTargetInput) =>
    api<ExportTarget>('PUT', `${TARGETS_PATH}/${encodeURIComponent(name)}`, input);

export const deleteExportTarget = (name: string) =>
    api<void>('DELETE', `${TARGETS_PATH}/${encodeURIComponent(name)}`);

export const testExportTarget = (name: string) =>
    api<void>('POST', `${TARGETS_PATH}/${encodeURIComponent(name)}/test`);

export const listExports = () => api<ExportRecord[]>('GET', '/exports');

export const getExport = (id: string) => api<ExportRecord>('GET', `/exports/${id}`);

export const createExport = (scope: ExportScope, target: string | null) =>
    api<{ id: string }>('POST', '/exports', { scope, target });

export const restoreExport = (id: string, options: RestoreOptions) =>
    api<Record<string, unknown>>('POST', `/exports/${id}/restore`, options);

export async function listScopeResources(kind: ScopeKind): Promise<ScopeResource[]> {
    switch (kind) {
        case 'resource_group':
            return listResourceGroups();
        case 'workload':
            return listWorkloads();
        case 'volume':
            return listVolumes();
        case 'bucket':
            return listBuckets();
        default:
            return [];
    }
}
