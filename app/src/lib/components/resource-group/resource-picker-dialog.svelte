<script lang="ts" module>
    export const RESOURCE_TYPES = [
        { key: "docker-container", label: "Docker Container", description: "Deploy a single container", icon: "logos:docker-icon" },
        { key: "docker-compose", label: "Docker Compose", description: "Deploy multiple related containers as one stack", icon: "logos:docker-icon" },
        { key: "vm", label: "Virtual Machine", description: "Boot a full VM from an ISO image", icon: "mdi:monitor" },
        { key: "volume", label: "Volume", description: "Add a block storage volume", icon: "mdi:database-outline" },
        { key: "bucket", label: "S3 Bucket", description: "Add an S3-compatible object storage bucket", icon: "fluent-emoji-high-contrast:bucket" },
    ] as const;

    export type ResourceTypeKey = (typeof RESOURCE_TYPES)[number]["key"];
</script>

<script lang="ts">
    import Icon from "@iconify/svelte";
    import ModalDialog from "./modal-dialog.svelte";

    let { onPick }: { onPick: (key: ResourceTypeKey) => void } = $props();

    let dialog = $state<ModalDialog | null>(null);

    export function open() {
        dialog?.open();
    }

    function pick(key: ResourceTypeKey) {
        dialog?.close();
        onPick(key);
    }
</script>

<ModalDialog bind:this={dialog} title="Add Resource">
    <div class="flex flex-col gap-2">
        {#each RESOURCE_TYPES as type}
            <button
                class="flex items-center gap-3 rounded-lg border p-3 text-left hover:bg-muted/40 transition-colors"
                onclick={() => pick(type.key)}
            >
                <div class="flex w-9 shrink-0 items-center justify-center">
                    <Icon icon={type.icon} width={20} height={20} />
                </div>
                <div>
                    <p class="text-sm font-medium leading-tight">{type.label}</p>
                    <p class="text-xs text-muted-foreground">{type.description}</p>
                </div>
            </button>
        {/each}
    </div>
</ModalDialog>
