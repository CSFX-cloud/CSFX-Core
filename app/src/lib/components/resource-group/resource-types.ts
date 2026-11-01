export const RESOURCE_CATEGORIES = [
    { id: "compute", label: "Compute" },
    { id: "containers", label: "Containers" },
    { id: "storage", label: "Storage" },
    { id: "networking", label: "Networking" },
    { id: "security", label: "Security" },
] as const;

export type ResourceCategory = (typeof RESOURCE_CATEGORIES)[number]["id"];

export const RESOURCE_TYPES = [
    { key: "vm", service: "virtual-machines", category: "compute", label: "Virtual Machines", description: "Boot a full VM from an ISO image.", equivalent: "EC2 · VIRTUAL MACHINE · COMPUTE ENGINE" },
    { key: "web-app", service: "web-apps", category: "compute", label: "Web Apps", description: "Build and host a site or app from a Git repository.", equivalent: "VERCEL · NETLIFY · COOLIFY · APP SERVICE" },
    { key: "docker-container", service: "docker-containers", category: "containers", label: "Docker Containers", description: "Deploy a single container.", equivalent: "ECS · CONTAINER INSTANCES · CLOUD RUN" },
    { key: "container-registry", service: "container-registry", category: "containers", label: "Container Registry", description: "Store and share container images for your deployments.", equivalent: "ECR · ACR · ARTIFACT REGISTRY · DOCKER HUB" },
    { key: "docker-compose", service: "docker-compose", category: "containers", label: "Docker Compose", description: "Deploy related containers as one stack.", equivalent: "COMPOSE.YML · MULTI-CONTAINER STACKS" },
    { key: "bucket", service: "object-storage", category: "storage", label: "Object Storage", description: "S3-compatible buckets for files and backups.", equivalent: "S3 · BLOB STORAGE · CLOUD STORAGE" },
    { key: "volume", service: "block-storage", category: "storage", label: "Block Storage", description: "Attach a volume to a VM or container.", equivalent: "EBS · MANAGED DISK · PERSISTENT DISK" },
    { key: "key-vault", service: "key-vault", category: "security", label: "Key Vault", description: "Store secrets that resources read at start.", equivalent: "KEY VAULT · SECRETS MANAGER · SECRET MANAGER" },
    { key: "api-gateway", service: "load-balancer", category: "networking", label: "API Gateway", description: "Publish services under local or public domains.", equivalent: "API GATEWAY · APIM · REVERSE PROXY" },
] as const;

export type ResourceTypeKey = (typeof RESOURCE_TYPES)[number]["key"];
