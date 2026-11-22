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
export const MAX_EXTRA_NICS = 3;
export const BYTES_PER_GIB = 1024 * 1024 * 1024;
export const MILLICORES_PER_VCPU = 1000;

const IPV4_PATTERN = /^((25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(25[0-5]|2[0-4]\d|1?\d?\d)$/;

export function isIpv4(value: string): boolean {
    return IPV4_PATTERN.test(value);
}

export function architectureLabel(id: string): string {
    return VM_ARCHITECTURES.find((architecture) => architecture.id === id)?.label ?? id;
}

const FIRST_GUEST_OFFSET = 3;

function ipv4ToNumber(value: string): number {
    return value.split(".").reduce((sum, octet) => sum * 256 + Number(octet), 0);
}

export function isGuestAddress(cidr: string, ip: string): boolean {
    const [network, prefixText] = cidr.split("/");
    const prefix = Number(prefixText);
    if (!isIpv4(network ?? "") || !isIpv4(ip) || !Number.isInteger(prefix) || prefix < 8 || prefix > 29) return false;
    const hostCount = 2 ** (32 - prefix);
    const offset = ipv4ToNumber(ip) - Math.floor(ipv4ToNumber(network) / hostCount) * hostCount;
    return offset >= FIRST_GUEST_OFFSET && offset < hostCount - 1;
}

export function guestAddressError(cidr: string, usedIps: string[], ips: string[]): string | null {
    const seen = new Set<string>();
    for (const ip of ips) {
        if (!isIpv4(ip)) return `${ip || "An empty address"} is not a valid IPv4 address`;
        if (!isGuestAddress(cidr, ip)) return `${ip} is not an assignable address in ${cidr}`;
        if (usedIps.includes(ip) || seen.has(ip)) return `${ip} is already in use`;
        seen.add(ip);
    }
    return null;
}
