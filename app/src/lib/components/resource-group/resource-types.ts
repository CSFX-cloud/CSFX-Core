export const RESOURCE_CATEGORIES = [
    { id: "compute", label: "Compute" },
    { id: "containers", label: "Containers" },
    { id: "storage", label: "Storage" },
    { id: "networking", label: "Networking" },
    { id: "security", label: "Security" },
] as const;

export type ResourceCategory = (typeof RESOURCE_CATEGORIES)[number]["id"];

export const RESOURCE_TYPES = [
    { key: "vm", service: "virtual-machines", category: "compute", label: "Virtual Machines", description: "Boot a full VM from an ISO image." },
    { key: "web-app", service: "web-apps", category: "compute", label: "Web Apps", description: "Build and host a site or app from a Git repository." },
    { key: "docker-container", service: "docker-containers", category: "containers", label: "Docker Containers", description: "Deploy a single container." },
    { key: "container-registry", service: "container-registry", category: "containers", label: "Container Registry", description: "Store and share container images for your deployments." },
    { key: "docker-compose", service: "docker-compose", category: "containers", label: "Docker Compose", description: "Deploy related containers as one stack." },
    { key: "bucket", service: "object-storage", category: "storage", label: "Object Storage", description: "S3-compatible buckets for files and backups." },
    { key: "volume", service: "block-storage", category: "storage", label: "Block Storage", description: "Attach a volume to a VM or container." },
    { key: "key-vault", service: "key-vault", category: "security", label: "Key Vault", description: "Store secrets that resources read at start." },
    { key: "api-gateway", service: "api-gateway", category: "networking", label: "API Gateway", description: "Publish services under local or public domains." },
] as const;

export type ResourceTypeKey = (typeof RESOURCE_TYPES)[number]["key"];
