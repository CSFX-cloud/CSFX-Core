import { api, openSocketUrl, request } from './http';

const API_BASE = '/api';

export interface ResourceGroup {
    id: string;
    organization_id: string;
    name: string;
    description: string | null;
    internal_cidr: string;
    status: string;
    icon: string;
    color: string;
    pinned: boolean;
    has_icon_image: boolean;
    preview_url: string | null;
    created_at: string;
    updated_at: string | null;
}

export interface CreateResourceGroupRequest {
    name: string;
    description?: string;
    internal_cidr: string;
    icon?: string;
    color?: string;
}

export interface UpdateResourceGroupRequest {
    name?: string;
    description?: string;
    icon?: string;
    color?: string;
    pinned?: boolean;
    preview_url?: string;
}

export interface PortMapping {
    container_port: number;
    protocol: string | null;
    rg_port: number | null;
    node_port: number | null;
}

export interface VolumeMount {
    volume_id: string;
    mount_path: string;
}

export interface UsageEntry {
    name: string;
    bytes: number;
}

export interface VolumeUsage {
    mount_path: string;
    used_bytes: number;
    total_bytes: number;
    entries: UsageEntry[];
}

export interface Volume {
    id: string;
    name: string;
    size_gb: number;
    pool: string;
    status: string;
    attached_to_workload: string | null;
    resource_group_id: string | null;
    created_at: string;
}

export interface Bucket {
    id: string;
    name: string;
    global_alias: string;
    exposure: 'internal' | 'external' | 'node_port';
    quota_max_size: number | null;
    quota_max_objects: number | null;
    status: string;
    resource_group_id: string | null;
    created_at: string;
    updated_at: string | null;
}

export interface BucketAccessKey {
    id: string;
    bucket_id: string;
    name: string;
    garage_key_id: string;
    permissions: string;
    expires_at: string | null;
    last_rotated_at: string | null;
    created_at: string;
}

export interface BucketAccessKeyCreated extends BucketAccessKey {
    secret_access_key: string;
}

export interface CreateBucketRequest {
    name: string;
    resource_group_id?: string;
    exposure?: 'internal' | 'external';
    quota_max_size?: number;
    quota_max_objects?: number;
}

export interface CreateBucketAccessKeyRequest {
    name: string;
    permissions?: string;
    expires_at?: string;
}

export interface UpdateBucketRequest {
    exposure?: 'internal' | 'external';
    quota_max_size?: number;
    quota_max_objects?: number;
}

export interface Workload {
    id: string;
    name: string;
    image: string;
    cpu_millicores: number;
    memory_bytes: number;
    disk_bytes: number;
    cpu_limit_millicores: number | null;
    memory_limit_bytes: number | null;
    qos_class: 'guaranteed' | 'burstable' | 'besteffort';
    auto_resources: boolean;
    guest_ip: string | null;
    recommended_cpu_millicores: number | null;
    recommended_memory_bytes: number | null;
    status: string;
    assigned_agent_id: string | null;
    container_id: string | null;
    env_vars: Record<string, string> | null;
    ports: PortMapping[] | null;
    volume_mounts: VolumeMount[] | null;
    volume_usage: VolumeUsage[] | null;
    resource_group_id: string | null;
    stack_id: string | null;
    service_name: string | null;
    restart_policy: string;
    max_restarts: number | null;
    restart_count: number;
    runtime_class: 'firecracker' | 'vm';
    desired_state: string;
    cpu_usage_percent: number | null;
    memory_usage_bytes: number | null;
    network_rx_bytes: number | null;
    network_tx_bytes: number | null;
    stats_updated_at: string | null;
    created_at: string;
    updated_at: string | null;
}

export interface CreateWorkloadRequest {
    name: string;
    image: string;
    cpu_millicores: number;
    memory_bytes: number;
    disk_bytes: number;
    cpu_limit_millicores?: number;
    memory_limit_bytes?: number;
    auto_resources?: boolean;
    replicas?: number;
    env_vars: Record<string, string> | null;
    ports: PortMapping[] | null;
    volume_mounts: VolumeMount[] | null;
    resource_group_id: string;
    restart_policy?: 'always' | 'on-failure' | 'never';
    max_restarts?: number | null;
    runtime_class?: 'firecracker' | 'vm';
}

export interface CreateWorkloadResult {
    workload_id: string;
    status: string;
    assigned_agent_id: string | null;
    message: string;
}

export interface UpdateWorkloadRequest {
    image?: string;
    env_vars?: Record<string, string> | null;
    ports?: PortMapping[] | null;
    restart_policy?: 'always' | 'on-failure' | 'never';
    max_restarts?: number | null;
    volume_mounts?: VolumeMount[];
}

export interface CreateVolumeRequest {
    name: string;
    size_gb: number;
    resource_group_id: string;
}

export interface CreateStackRequest {
    name: string;
    resource_group_id: string;
    compose_yaml: string;
}

export interface CreateStackWorkloadResult {
    workload_id: string;
    status: string;
    assigned_agent_id: string | null;
    message: string;
}

export interface CreateStackResponse {
    stack_id: string;
    workloads: CreateStackWorkloadResult[];
}

export const listResourceGroups = () =>
    api<ResourceGroup[]>('GET', '/resource-groups');

export const suggestCidr = async (): Promise<string> =>
    (await api<{ internal_cidr: string }>('GET', '/resource-groups/suggest-cidr')).internal_cidr;

export const getResourceGroup = (id: string) =>
    api<ResourceGroup>('GET', `/resource-groups/${id}`);

export const createResourceGroup = (req: CreateResourceGroupRequest) =>
    api<ResourceGroup>('POST', '/resource-groups', req);

export const updateResourceGroup = (id: string, req: UpdateResourceGroupRequest) =>
    api<ResourceGroup>('PATCH', `/resource-groups/${id}`, req);

export function resourceGroupIconImageUrl(id: string): string {
    return `${API_BASE}/resource-groups/${id}/icon-image`;
}

export const uploadResourceGroupIconImage = (id: string, file: File) =>
    api<ResourceGroup>('PUT', `/resource-groups/${id}/icon-image`, file);

export const deleteResourceGroupIconImage = (id: string) =>
    api<ResourceGroup>('DELETE', `/resource-groups/${id}/icon-image`);

export const deleteResourceGroup = (id: string) =>
    api<void>('DELETE', `/resource-groups/${id}`);

export const listResourceGroupWorkloads = (rgId: string) =>
    api<Workload[]>('GET', `/resource-groups/${rgId}/workloads`);

export const listWorkloads = () =>
    api<Workload[]>('GET', '/workloads');

export const createWorkload = (req: CreateWorkloadRequest) =>
    api<CreateWorkloadResult | CreateWorkloadResult[]>('POST', '/workloads', req);

export const createWorkloadStack = (req: CreateStackRequest) =>
    api<CreateStackResponse>('POST', '/workload-stacks', req);

export interface Stack {
    id: string;
    resource_group_id: string;
    name: string;
    compose_source: string | null;
    status: string;
    created_at: string;
    updated_at: string | null;
}

export const getStack = (id: string) =>
    api<Stack>('GET', `/workload-stacks/${id}`);

export const deleteStack = (id: string) =>
    api<void>('DELETE', `/workload-stacks/${id}`);

export const stopStack = (id: string) =>
    api<void>('POST', `/workload-stacks/${id}/stop`);

export const restartStack = (id: string) =>
    api<void>('POST', `/workload-stacks/${id}/restart`);

export const redeployStack = (id: string, compose_yaml: string) =>
    api<void>('PATCH', `/workload-stacks/${id}`, { compose_yaml });

export const deleteWorkload = (id: string) =>
    api<void>('DELETE', `/workloads/${id}`);

export const updateWorkload = (id: string, req: UpdateWorkloadRequest) =>
    api<Workload>('PATCH', `/workloads/${id}`, req);

export const stopWorkload = (id: string) =>
    api<Workload>('POST', `/workloads/${id}/stop`);

export const restartWorkload = (id: string) =>
    api<Workload>('POST', `/workloads/${id}/restart`);

export async function streamWorkloadLogs(id: string, signal: AbortSignal): Promise<ReadableStream<Uint8Array>> {
    const res = await request('GET', `/workloads/${id}/logs`, undefined, signal);
    if (!res.body) throw new Error('Log stream has no body');
    return res.body;
}

export async function openWorkloadExecSocket(id: string): Promise<WebSocket> {
    return new WebSocket(await openSocketUrl(`/workloads/${id}/exec/ticket`, `/workloads/${id}/exec`));
}

export const getWorkloadVncUrl = (id: string) => openSocketUrl(`/workloads/${id}/vnc/ticket`, `/workloads/${id}/vnc`);

export const listResourceGroupVolumes = (rgId: string) =>
    api<Volume[]>('GET', `/resource-groups/${rgId}/volumes`);

export const listVolumes = () =>
    api<Volume[]>('GET', '/volumes');

export const createVolume = (req: CreateVolumeRequest) =>
    api<Volume>('POST', '/volumes', req);

export const resizeVolume = (id: string, sizeGb: number) =>
    api<Volume>('POST', `/volumes/${id}/resize`, { size_gb: sizeGb });

export const deleteVolume = (id: string) =>
    api<void>('DELETE', `/volumes/${id}`);

export const listBuckets = () =>
    api<Bucket[]>('GET', '/buckets');

export const getBucket = (id: string) =>
    api<Bucket>('GET', `/buckets/${id}`);

export const listResourceGroupBuckets = (rgId: string) =>
    api<Bucket[]>('GET', `/resource-groups/${rgId}/buckets`);

export const createBucket = (req: CreateBucketRequest) =>
    api<Bucket>('POST', '/buckets', req);

export const updateBucket = (id: string, req: UpdateBucketRequest) =>
    api<Bucket>('PATCH', `/buckets/${id}`, req);

export const deleteBucket = (id: string) =>
    api<void>('DELETE', `/buckets/${id}`);

export const listBucketKeys = (bucketId: string) =>
    api<BucketAccessKey[]>('GET', `/buckets/${bucketId}/keys`);

export const createBucketKey = (bucketId: string, req: CreateBucketAccessKeyRequest) =>
    api<BucketAccessKeyCreated>('POST', `/buckets/${bucketId}/keys`, req);

export const deleteBucketKey = (bucketId: string, keyId: string) =>
    api<void>('DELETE', `/buckets/${bucketId}/keys/${keyId}`);

export interface ObjectEntry {
    key: string;
    size: number;
    last_modified: string;
}

export interface ListObjectsResult {
    objects: ObjectEntry[];
    folders: string[];
    next_continuation_token: string | null;
}

export interface PresignResult {
    url: string;
    expires_in_seconds: number;
}

export const listBucketObjects = (bucketId: string, prefix: string) =>
    api<ListObjectsResult>('GET', `/buckets/${bucketId}/objects?prefix=${encodeURIComponent(prefix)}`);

export const deleteBucketObject = (bucketId: string, key: string) =>
    api<void>('DELETE', `/buckets/${bucketId}/objects/${key}`);

export const presignObjectUpload = (bucketId: string, key: string) =>
    api<PresignResult>('POST', `/buckets/${bucketId}/objects/presign-upload`, { key });

const ISO_BUCKET_NAME = 'vm-isos';

export async function findIsoBucket(rgId: string): Promise<Bucket | null> {
    const buckets = await listResourceGroupBuckets(rgId);
    return buckets.find((b) => b.name === ISO_BUCKET_NAME) ?? null;
}

export async function ensureIsoBucket(rgId: string): Promise<Bucket> {
    const existing = await findIsoBucket(rgId);
    if (existing) return existing;

    return createBucket({
        name: ISO_BUCKET_NAME,
        resource_group_id: rgId,
        exposure: 'internal',
    });
}

export async function uploadIso(
    bucketId: string,
    file: File,
    onProgress?: (fraction: number) => void
): Promise<string> {
    const key = `${crypto.randomUUID()}-${file.name}`;
    const { url } = await presignObjectUpload(bucketId, key);

    await new Promise<void>((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open('PUT', url);
        xhr.upload.onprogress = (event) => {
            if (event.lengthComputable) onProgress?.(event.loaded / event.total);
        };
        xhr.onload = () => {
            if (xhr.status >= 200 && xhr.status < 300) resolve();
            else reject(new Error(`Failed to upload iso: ${xhr.status}`));
        };
        xhr.onerror = () => reject(new Error('Failed to upload iso'));
        xhr.send(file);
    });

    const { url: downloadUrl } = await presignObjectDownload(bucketId, key);
    return downloadUrl;
}

export const presignObjectDownload = (bucketId: string, key: string) =>
    api<PresignResult>('GET', `/buckets/${bucketId}/objects/presign-download/${key}`);

export async function uploadObjectToPresignedUrl(url: string, file: File): Promise<void> {
    const res = await fetch(url, {
        method: 'PUT',
        body: file,
    });
    if (!res.ok) throw new Error(`Upload failed: ${res.status}`);
}
