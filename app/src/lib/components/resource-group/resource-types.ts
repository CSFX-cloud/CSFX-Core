export const RESOURCE_TYPES = [
    { key: "docker-container", label: "Docker Container", description: "Deploy a single container", icon: "logos:docker-icon" },
    { key: "docker-compose", label: "Docker Compose", description: "Deploy multiple related containers as one stack", icon: "logos:docker-icon" },
    { key: "vm", label: "Virtual Machine", description: "Boot a full VM from an ISO image", icon: "mdi:monitor" },
    { key: "volume", label: "Volume", description: "Add a block storage volume", icon: "mdi:database-outline" },
    { key: "bucket", label: "S3 Bucket", description: "Add an S3-compatible object storage bucket", icon: "fluent-emoji-high-contrast:bucket" },
] as const;

export type ResourceTypeKey = (typeof RESOURCE_TYPES)[number]["key"];
