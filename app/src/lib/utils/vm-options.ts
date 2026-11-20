export interface VmOption {
    id: string;
    label: string;
    detail: string;
}

export const VM_ARCHITECTURES: VmOption[] = [
    { id: "x86_64", label: "x86-64", detail: "Intel and AMD" },
    { id: "aarch64", label: "ARM64", detail: "aarch64" },
];

export const NIC_MODELS: VmOption[] = [
    { id: "virtio", label: "VirtIO", detail: "Fastest, needs guest drivers (included in Linux)" },
    { id: "e1000e", label: "Intel E1000E", detail: "Works out of the box with most operating systems" },
];

export const DEFAULT_NIC_MODEL = "virtio";

export function architectureLabel(id: string): string {
    return VM_ARCHITECTURES.find((architecture) => architecture.id === id)?.label ?? id;
}
