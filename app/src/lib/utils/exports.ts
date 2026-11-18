import type { ExportStatus, ScopeKind } from '$lib/api/exports';

export const SCOPES: { kind: ScopeKind; label: string; description: string; needsResource: boolean }[] = [
    { kind: 'config', label: 'Configuration', description: 'Settings, users, roles, resource groups and networks. No tenant data.', needsResource: false },
    { kind: 'full', label: 'Everything', description: 'Configuration, database, etcd and all tenant data.', needsResource: false },
    { kind: 'resource_group', label: 'Resource group', description: 'Workloads, volumes and buckets of one resource group.', needsResource: true },
    { kind: 'workload', label: 'Workload', description: 'One workload and the boot disk of a vm.', needsResource: true },
    { kind: 'volume', label: 'Volume', description: 'One volume image.', needsResource: true },
    { kind: 'bucket', label: 'Bucket', description: 'All objects of one bucket.', needsResource: true },
];

export const RESTORABLE_SCOPES: ScopeKind[] = ['config', 'resource_group', 'workload', 'volume', 'bucket'];

export function scopeLabel(kind: ScopeKind): string {
    return SCOPES.find((scope) => scope.kind === kind)?.label ?? kind;
}

export function statusBadge(status: ExportStatus): string {
    if (status === 'completed') return 'cx-badge--success';
    if (status === 'failed') return 'cx-badge--danger';
    return 'cx-badge--info';
}
