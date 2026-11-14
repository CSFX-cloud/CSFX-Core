import { listGateways, type Gateway } from "$lib/api/ingress";
import { listRegistries, type Registry } from "$lib/api/registries";
import {
    listResourceGroupBuckets,
    listResourceGroupVolumes,
    listResourceGroupWorkloads,
    stopWorkload,
    type Bucket,
    type Volume,
    type Workload,
} from "$lib/api/resource-groups";
import { listSites, type SiteSummary } from "$lib/api/sites";
import { listVaults, type Vault } from "$lib/api/vaults";

const ACTIVE_STATUSES = new Set(["pending", "scheduled", "pulling", "creating", "starting", "running"]);
const STOP_POLL_INTERVAL_MS = 1000;
const STOP_POLL_ATTEMPTS = 60;
const ID_PREVIEW_LENGTH = 8;

export interface GroupInventory {
    workloads: Workload[];
    volumes: Volume[];
    buckets: Bucket[];
    vaults: Vault[];
    sites: SiteSummary[];
    registries: Registry[];
    gateways: Gateway[];
}

export interface DeleteEntry {
    type: string;
    name: string;
    status: string;
}

export interface DeletePreview {
    entries: DeleteEntry[];
    dependencies: string[];
    running: Workload[];
    vmCount: number;
    volumeCount: number;
}

export function isActive(workload: Workload): boolean {
    return ACTIVE_STATUSES.has(workload.status);
}

export function displayName(workload: Workload): string {
    return workload.service_name ?? workload.name;
}

export async function loadGroupInventory(groupId: string): Promise<GroupInventory> {
    const [workloads, volumes, buckets, vaults, sites, registries, gateways] = await Promise.all([
        listResourceGroupWorkloads(groupId),
        listResourceGroupVolumes(groupId),
        listResourceGroupBuckets(groupId),
        listVaults(groupId).catch((): Vault[] => []),
        listSites(groupId).catch((): SiteSummary[] => []),
        listRegistries(groupId).catch((): Registry[] => []),
        listGateways(groupId),
    ]);
    return { workloads, volumes, buckets, vaults, sites, registries, gateways };
}

function workloadEntry(workload: Workload): DeleteEntry {
    return { type: workload.runtime_class === "vm" ? "VM" : "Container", name: displayName(workload), status: workload.status };
}

function inventoryEntries(inventory: GroupInventory): DeleteEntry[] {
    return [
        ...inventory.workloads.map(workloadEntry),
        ...inventory.volumes.map((volume): DeleteEntry => ({ type: "Volume", name: volume.name, status: volume.status })),
        ...inventory.buckets.map((bucket): DeleteEntry => ({ type: "Bucket", name: bucket.name, status: bucket.status })),
        ...inventory.vaults.map((vault): DeleteEntry => ({ type: "Vault", name: vault.name, status: vault.sealed ? "sealed" : "active" })),
        ...inventory.sites.map((site): DeleteEntry => ({ type: "Web app", name: site.name, status: site.latest?.status ?? "no deployment" })),
        ...inventory.registries.map((registry): DeleteEntry => ({ type: "Registry", name: registry.name, status: "active" })),
        ...inventory.gateways.map((gateway): DeleteEntry => ({ type: "Gateway", name: gateway.name, status: gateway.status ?? "unknown" })),
    ];
}

function volumeDependencies(volumes: Volume[], workloads: Workload[]): string[] {
    const names = new Map(workloads.map((workload) => [workload.id, displayName(workload)]));
    return volumes
        .filter((volume) => volume.attached_to_workload)
        .map((volume) => {
            const owner = names.get(volume.attached_to_workload ?? "") ?? `workload ${volume.attached_to_workload?.slice(0, ID_PREVIEW_LENGTH)}`;
            return `Volume ${volume.name} is attached to ${owner}`;
        });
}

function gatewayDependencies(gateways: Gateway[], workloads: Workload[]): string[] {
    const names = new Map(workloads.map((workload) => [workload.id, displayName(workload)]));
    return gateways.map((gateway) => `Gateway ${gateway.name} routes to ${names.get(gateway.workload_id) ?? "a workload"}`);
}

export function groupDeletePreview(inventory: GroupInventory, cidr: string): DeletePreview {
    const entries = inventoryEntries(inventory);
    return {
        entries,
        dependencies: [
            ...volumeDependencies(inventory.volumes, inventory.workloads),
            ...gatewayDependencies(inventory.gateways, inventory.workloads),
            `Private network ${cidr} is shared by ${entries.length} ${entries.length === 1 ? "resource" : "resources"}`,
        ],
        running: inventory.workloads.filter(isActive),
        vmCount: inventory.workloads.filter((workload) => workload.runtime_class === "vm").length,
        volumeCount: inventory.volumes.length,
    };
}

export function workloadDeletePreview(workload: Workload, inventory: GroupInventory): DeletePreview {
    const mounted = new Set((workload.volume_mounts ?? []).map((mount) => mount.volume_id));
    const volumes = inventory.volumes.filter((volume) => mounted.has(volume.id));
    const gateways = inventory.gateways.filter((gateway) => gateway.workload_id === workload.id);
    return {
        entries: [workloadEntry(workload)],
        dependencies: [
            ...volumes.map((volume) => `Volume ${volume.name} is mounted and will be detached`),
            ...gateways.map((gateway) => `Gateway ${gateway.name} routes to this workload`),
        ],
        running: isActive(workload) ? [workload] : [],
        vmCount: workload.runtime_class === "vm" ? 1 : 0,
        volumeCount: 0,
    };
}

export function stackDeletePreview(children: Workload[], inventory: GroupInventory): DeletePreview {
    const previews = children.map((child) => workloadDeletePreview(child, inventory));
    return {
        entries: previews.flatMap((preview) => preview.entries),
        dependencies: previews.flatMap((preview) => preview.dependencies),
        running: previews.flatMap((preview) => preview.running),
        vmCount: previews.reduce((sum, preview) => sum + preview.vmCount, 0),
        volumeCount: 0,
    };
}

export function singleDeletePreview(entry: DeleteEntry, dependencies: string[] = []): DeletePreview {
    return { entries: [entry], dependencies, running: [], vmCount: 0, volumeCount: entry.type === "Volume" ? 1 : 0 };
}

export function volumeDeletePreview(volume: Volume, inventory: GroupInventory): DeletePreview {
    const dependencies = volumeDependencies([volume], inventory.workloads);
    return singleDeletePreview({ type: "Volume", name: volume.name, status: volume.status }, dependencies);
}

function pause(): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, STOP_POLL_INTERVAL_MS));
}

export async function stopAndWait(groupId: string, workloads: Workload[]): Promise<void> {
    await Promise.all(workloads.map((workload) => stopWorkload(workload.id)));
    const ids = new Set(workloads.map((workload) => workload.id));
    for (let attempt = 0; attempt < STOP_POLL_ATTEMPTS; attempt++) {
        const current = await listResourceGroupWorkloads(groupId);
        if (!current.some((workload) => ids.has(workload.id) && isActive(workload))) return;
        await pause();
    }
    throw new Error("Timed out waiting for workloads to stop");
}
