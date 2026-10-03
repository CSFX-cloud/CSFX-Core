<script lang="ts">
    import Icon from "@iconify/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { updateResourceGroup, type ResourceGroup } from "$lib/api/resource-groups";
    import RgIcon from "$lib/components/rg-icon.svelte";
    import StatusBadge from "$lib/components/status-badge.svelte";

    interface ResourceSummary {
        label: string;
        running: number;
        total: number;
    }

    let {
        group,
        resources,
        onUpdated,
        onEditIcon,
        onDelete,
        onError,
    }: {
        group: ResourceGroup;
        resources: ResourceSummary[];
        onUpdated: (group: ResourceGroup) => void;
        onEditIcon: () => void;
        onDelete: () => void;
        onError: (message: string) => void;
    } = $props();

    let editingField = $state<"name" | "description" | null>(null);
    let editValue = $state("");
    let downloadingVpn = $state(false);

    async function patch(changes: Parameters<typeof updateResourceGroup>[2]) {
        if (!auth.token) return;
        try {
            onUpdated(await updateResourceGroup(auth.token, group.id, changes));
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to update resource group");
        }
    }

    function startEdit(field: "name" | "description") {
        editValue = field === "name" ? group.name : (group.description ?? "");
        editingField = field;
    }

    async function commitEdit() {
        if (!editingField) return;
        const field = editingField;
        const value = editValue.trim();
        editingField = null;
        if (field === "name" && !value) return;
        if (value === (field === "name" ? group.name : (group.description ?? ""))) return;
        await patch({ [field]: value });
    }

    async function downloadVpnConfig() {
        if (!auth.token) return;
        downloadingVpn = true;
        try {
            const response = await fetch(`/api/resource-groups/${group.id}/vpn-config`, {
                headers: { Authorization: `Bearer ${auth.token}` },
            });
            if (!response.ok) {
                const body = await response.json().catch(() => ({}));
                throw new Error(body.error ?? `HTTP ${response.status}`);
            }
            const url = URL.createObjectURL(await response.blob());
            const link = document.createElement("a");
            const disposition = response.headers.get("content-disposition") ?? "";
            link.href = url;
            link.download = disposition.match(/filename="([^"]+)"/)?.[1] ?? "csfx-vpn.conf";
            link.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            onError(e instanceof Error ? e.message : "VPN config download failed");
        } finally {
            downloadingVpn = false;
        }
    }

    function focusOnMount(node: HTMLInputElement) {
        node.focus();
        node.select();
    }

    function handleEditKeydown(event: KeyboardEvent) {
        if (event.key === "Enter") commitEdit();
        if (event.key === "Escape") editingField = null;
    }
</script>

        <div class="flex items-center gap-2">
            <button
                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border transition-opacity hover:opacity-80 overflow-hidden"
                style="background-color: {group.color}20; color: {group.color};"
                onclick={onEditIcon}
                aria-label="Edit icon"
                title="Edit icon"
            >
                <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={18} />
            </button>
            {#if editingField === "name"}
                <input
                    class="min-w-0 flex-1 border rounded px-2 py-1 text-sm bg-background"
                    bind:value={editValue}
                    onblur={commitEdit}
                    onkeydown={handleEditKeydown}
                    use:focusOnMount
                />
            {:else}
                <button
                    class="min-w-0 flex-1 text-left text-lg font-light text-foreground/70 truncate rounded px-1 hover:bg-muted transition-colors"
                    onclick={() => startEdit("name")}
                    title="Edit name"
                >{group.name}</button>
            {/if}
        </div>
        {#if editingField === "description"}
            <input
                class="mt-2 w-full border rounded px-2 py-1 text-xs bg-background"
                placeholder="Description"
                bind:value={editValue}
                onblur={commitEdit}
                onkeydown={handleEditKeydown}
                use:focusOnMount
            />
        {:else}
            <button
                class="mt-2 w-full text-left text-xs text-muted-foreground rounded px-1 py-0.5 hover:bg-muted transition-colors"
                onclick={() => startEdit("description")}
                title="Edit description"
            >{group.description || "Add description"}</button>
        {/if}

        <div class="flex flex-col gap-3 mt-5 pb-5 border-b border-dashed border-border">
            <div class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <Icon icon="mdi:pulse" width={14} height={14} />
                    Status
                </span>
                <StatusBadge status={group.status} />
            </div>
            <div class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <Icon icon="mdi:calendar-outline" width={14} height={14} />
                    Created
                </span>
                <span class="text-xs">{group.created_at.slice(0, 10)}</span>
            </div>
            <div class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <Icon icon="mdi:lan" width={14} height={14} />
                    Network
                </span>
                <span class="text-xs font-mono px-1.5 py-0.5 rounded-full border">{group.internal_cidr}</span>
            </div>
            <button
                class="flex items-center justify-between gap-2 rounded px-1 -mx-1 py-0.5 hover:bg-muted transition-colors"
                onclick={() => patch({ pinned: !group.pinned })}
                aria-label={group.pinned ? "Unpin" : "Pin"}
                title={group.pinned ? "Unpin" : "Pin"}
            >
                <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <Icon icon={group.pinned ? "mdi:pin" : "mdi:pin-outline"} width={14} height={14} />
                    Pinned
                </span>
                <span class="text-xs">{group.pinned ? "Yes" : "No"}</span>
            </button>
        </div>

        <div class="flex flex-col gap-3 mt-5">
            <h3 class="text-xs font-semibold">Resources</h3>
            {#each resources as row}
                <div class="flex flex-col gap-1">
                    <div class="flex items-center justify-between">
                        <span class="text-xs text-muted-foreground">{row.label}</span>
                        <span class="text-xs text-muted-foreground">{row.running}/{row.total}</span>
                    </div>
                    <div class="h-1.5 rounded-full bg-muted overflow-hidden">
                        <div
                            class="h-full rounded-full bg-green-500"
                            style="width: {row.total === 0 ? 0 : (row.running / row.total) * 100}%"
                        ></div>
                    </div>
                </div>
            {/each}
        </div>

        <button
            class="mt-auto flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md border text-xs font-medium hover:bg-muted transition-colors disabled:opacity-50"
            onclick={downloadVpnConfig}
            disabled={downloadingVpn}
        >
            <Icon icon="mdi:shield-outline" width={14} height={14} />
            {downloadingVpn ? "Generating..." : "Connect VPN"}
        </button>

        <button
            class="mt-2 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md border border-destructive/40 text-destructive text-xs font-medium hover:bg-destructive/10 transition-colors"
            onclick={onDelete}
        >
            <Icon icon="mdi:trash-can-outline" width={14} height={14} />
            Delete
        </button>
