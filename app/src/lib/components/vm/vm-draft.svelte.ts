import {
    bucketImageReference,
    createWorkload,
    ensureIsoBucket,
    findIsoBucket,
    listBucketObjects,
    uploadIso,
    type ObjectEntry,
} from "$lib/api/resource-groups";
import { BYTES_PER_GIB, DEFAULT_NIC_MODEL, guestAddressError, MAX_EXTRA_NICS, MILLICORES_PER_VCPU } from "$lib/utils/vm-options";

export type IsoMode = "existing" | "upload";
export type AddressMode = "dhcp" | "static";

export interface NicDraft {
    mode: AddressMode;
    ip: string;
}

function requestedIp(nic: NicDraft): string | null {
    return nic.mode === "static" ? nic.ip.trim() : null;
}

export class VmDraft {
    name = $state("");
    isoMode = $state<IsoMode>("upload");
    isoFile = $state<File | null>(null);
    existingIsos = $state<ObjectEntry[]>([]);
    existingLoading = $state(false);
    selectedIsoKey = $state("");
    isoBucketId = $state<string | null>(null);
    architecture = $state("");
    vcpus = $state(2);
    memoryGb = $state(2);
    diskGb = $state(20);
    nicModel = $state(DEFAULT_NIC_MODEL);
    nics = $state<NicDraft[]>([{ mode: "dhcp", ip: "" }]);
    volumeIds = $state<string[]>([]);
    uploadProgress = $state(0);
    deploying = $state(false);

    imageReady = $derived(this.isoMode === "upload" ? this.isoFile !== null : this.selectedIsoKey !== "");
    imageLabel = $derived(this.isoMode === "upload" ? (this.isoFile?.name ?? "") : isoFileName(this.selectedIsoKey));
    nameReady = $derived(this.name.trim() !== "");
    computeReady = $derived(this.architecture !== "" && this.vcpus >= 1 && this.memoryGb >= 0.5);
    storageReady = $derived(this.diskGb >= 1);
    staticIps = $derived(this.nics.map(requestedIp).filter((ip): ip is string => ip !== null));
    canAddNic = $derived(this.nics.length <= MAX_EXTRA_NICS);

    async loadExistingIsos(rgId: string) {
        this.existingLoading = true;
        try {
            const bucket = await findIsoBucket(rgId);
            this.isoBucketId = bucket?.id ?? null;
            this.existingIsos = bucket ? (await listBucketObjects(bucket.id, "")).objects : [];
            this.isoMode = this.existingIsos.length > 0 ? "existing" : "upload";
            this.selectedIsoKey = this.existingIsos[0]?.key ?? "";
        } finally {
            this.existingLoading = false;
        }
    }

    networkError(cidr: string, usedIps: string[]): string | null {
        return guestAddressError(cidr, usedIps, this.staticIps);
    }

    addNic() {
        if (this.canAddNic) this.nics = [...this.nics, { mode: "dhcp", ip: "" }];
    }

    removeNic(index: number) {
        this.nics = this.nics.filter((_, position) => position !== index);
    }

    toggleVolume(volumeId: string) {
        this.volumeIds = this.volumeIds.includes(volumeId)
            ? this.volumeIds.filter((id) => id !== volumeId)
            : [...this.volumeIds, volumeId];
    }

    private uploaded: { file: File; reference: string } | null = null;

    private async resolveImage(rgId: string): Promise<string> {
        if (this.isoMode === "existing" && this.isoBucketId) {
            return bucketImageReference(this.isoBucketId, this.selectedIsoKey);
        }
        const file = this.isoFile!;
        if (this.uploaded?.file === file) return this.uploaded.reference;
        const bucket = await ensureIsoBucket(rgId);
        const reference = await uploadIso(bucket.id, file, (fraction) => {
            this.uploadProgress = fraction;
        });
        this.uploaded = { file, reference };
        return reference;
    }

    async deploy(rgId: string) {
        this.deploying = true;
        this.uploadProgress = 0;
        try {
            const image = await this.resolveImage(rgId);
            await createWorkload({
                name: this.name.trim(),
                image,
                cpu_millicores: this.vcpus * MILLICORES_PER_VCPU,
                memory_bytes: Math.round(this.memoryGb * BYTES_PER_GIB),
                disk_bytes: this.diskGb * BYTES_PER_GIB,
                env_vars: null,
                ports: null,
                volume_mounts: this.volumeIds.map((volumeId, index) => ({
                    volume_id: volumeId,
                    mount_path: `/mnt/data${index + 1}`,
                })),
                resource_group_id: rgId,
                runtime_class: "vm",
                vm_config: {
                    architecture: this.architecture,
                    nic_model: this.nicModel,
                    guest_ip: requestedIp(this.nics[0]),
                    extra_nics: this.nics.slice(1).map((nic) => ({ ip: requestedIp(nic) })),
                },
            });
        } finally {
            this.deploying = false;
        }
    }
}

export function isoFileName(key: string): string {
    const parts = key.split("-");
    return parts.length > 5 ? parts.slice(5).join("-") : key;
}
