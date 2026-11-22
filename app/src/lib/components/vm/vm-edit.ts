import type { UpdateWorkloadRequest, Workload } from "$lib/api/resource-groups";
import { BYTES_PER_GIB, DEFAULT_NIC_MODEL, MILLICORES_PER_VCPU } from "$lib/utils/vm-options";

export interface VmEditForm {
    vcpus: number;
    memoryGb: number;
    diskGb: number;
    nicModel: string;
    addresses: string[];
    volumeIds: string[];
}

export function currentAddresses(workload: Workload): string[] {
    const extras = (workload.vm_config?.extra_nics ?? []).map((nic) => nic.ip ?? "");
    return [workload.guest_ip ?? "", ...extras];
}

export function initialForm(workload: Workload): VmEditForm {
    return {
        vcpus: workload.cpu_millicores / MILLICORES_PER_VCPU,
        memoryGb: workload.memory_bytes / BYTES_PER_GIB,
        diskGb: workload.disk_bytes / BYTES_PER_GIB,
        nicModel: workload.vm_config?.nic_model ?? DEFAULT_NIC_MODEL,
        addresses: currentAddresses(workload),
        volumeIds: (workload.volume_mounts ?? []).map((mount) => mount.volume_id),
    };
}

function sameAddresses(left: string[], right: string[]): boolean {
    return left.length === right.length && left.every((ip, index) => ip.trim() === right[index]);
}

function sameVolumes(left: string[], right: string[]): boolean {
    return left.length === right.length && left.every((id) => right.includes(id));
}

export function vmUpdateRequest(workload: Workload, form: VmEditForm): UpdateWorkloadRequest {
    const request: UpdateWorkloadRequest = {};
    const cpu = form.vcpus * MILLICORES_PER_VCPU;
    const memory = Math.round(form.memoryGb * BYTES_PER_GIB);
    const disk = Math.round(form.diskGb * BYTES_PER_GIB);
    if (cpu !== workload.cpu_millicores) request.cpu_millicores = cpu;
    if (memory !== workload.memory_bytes) request.memory_bytes = memory;
    if (disk !== workload.disk_bytes) request.disk_bytes = disk;
    const modelChanged = form.nicModel !== (workload.vm_config?.nic_model ?? DEFAULT_NIC_MODEL);
    if (modelChanged || !sameAddresses(form.addresses, currentAddresses(workload))) {
        const [primary, ...extras] = form.addresses.map((ip) => ip.trim() || null);
        request.vm_config = {
            architecture: workload.vm_config?.architecture,
            nic_model: form.nicModel,
            guest_ip: primary ?? null,
            extra_nics: extras.map((ip) => ({ ip })),
        };
    }
    if (!sameVolumes(form.volumeIds, initialForm(workload).volumeIds)) {
        request.volume_mounts = form.volumeIds.map((volumeId, index) => ({ volume_id: volumeId, mount_path: `/mnt/data${index + 1}` }));
    }
    return request;
}
