<script lang="ts">
    import { Calendar, Network, Pin, Plus, ShieldCheck, Trash2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { updateResourceGroup, type ResourceGroup } from "$lib/api/resource-groups";
    import GroupTile from "$lib/components/resource-group/group-tile.svelte";
    import type { GroupHealth } from "$lib/utils/group-health";

    type EditField = "name" | "description";

    type Props = {
        group: ResourceGroup;
        base: string;
        health: GroupHealth;
        onupdated: (group: ResourceGroup) => void;
        onerror: (message: string) => void;
        oneditappearance: () => void;
        ondelete: () => void;
    };

    let { group, base, health, onupdated, onerror, oneditappearance, ondelete }: Props = $props();

    let editing = $state<EditField | null>(null);
    let editValue = $state("");
    let downloadingVpn = $state(false);

    async function patch(changes: Parameters<typeof updateResourceGroup>[2]) {
        if (!auth.token) return;
        try {
            onupdated(await updateResourceGroup(auth.token, group.id, changes));
        } catch (e) {
            onerror(e instanceof Error ? e.message : "Failed to update resource group");
        }
    }

    function startEdit(field: EditField) {
        editValue = field === "name" ? group.name : (group.description ?? "");
        editing = field;
    }

    async function commitEdit() {
        const field = editing;
        if (!field) return;
        const value = editValue.trim();
        editing = null;
        const current = field === "name" ? group.name : (group.description ?? "");
        if ((field === "name" && !value) || value === current) return;
        await patch({ [field]: value });
    }

    function handleEditKey(event: KeyboardEvent) {
        if (event.key === "Enter") commitEdit();
        if (event.key === "Escape") editing = null;
    }

    function focusOnMount(node: HTMLInputElement) {
        node.focus();
        node.select();
    }

    async function downloadVpnConfig() {
        if (!auth.token) return;
        downloadingVpn = true;
        try {
            const response = await fetch(`/api/resource-groups/${group.id}/vpn-config`, { headers: { Authorization: `Bearer ${auth.token}` } });
            if (!response.ok) {
                const body = await response.json().catch(() => ({}));
                throw new Error(body.error ?? `HTTP ${response.status}`);
            }
            const url = URL.createObjectURL(await response.blob());
            const link = document.createElement("a");
            link.href = url;
            link.download = (response.headers.get("content-disposition") ?? "").match(/filename="([^"]+)"/)?.[1] ?? "csfx-vpn.conf";
            link.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            onerror(e instanceof Error ? e.message : "VPN config download failed");
        } finally {
            downloadingVpn = false;
        }
    }
</script>

<section class="cx-stage" aria-label="Resource group {group.name}">
    <div class="cx-stage__canvas"></div>
    <div class="gd-head">
        <div class="gd-id">
            <button class="gd-tile" type="button" aria-label="Edit icon" title="Edit icon" onclick={oneditappearance}>
                <GroupTile {group} size="lg" />
            </button>
            <div style="min-width: 0">
                <div class="gd-title">
                    {#if editing === "name"}
                        <input class="cx-input gd-edit" aria-label="Group name" bind:value={editValue} onblur={commitEdit} onkeydown={handleEditKey} use:focusOnMount />
                    {:else}
                        <button class="gd-name" type="button" title="Edit name" onclick={() => startEdit("name")}>{group.name}</button>
                    {/if}
                    <span class="cx-badge {health.tone ? `cx-badge--${health.tone}` : ''}">{health.label}</span>
                </div>
                {#if editing === "description"}
                    <input class="cx-input gd-edit" style="margin-top: 6px" aria-label="Description" placeholder="Description" bind:value={editValue} onblur={commitEdit} onkeydown={handleEditKey} use:focusOnMount />
                {:else}
                    <button class="gd-desc" type="button" onclick={() => startEdit("description")}>{group.description || "Add a description"}</button>
                {/if}
                <div class="gd-meta">
                    <span><Network class="cx-i" aria-hidden="true" />{group.internal_cidr}</span>
                    <span><Calendar class="cx-i" aria-hidden="true" />Created {group.created_at.slice(0, 10)}</span>
                </div>
            </div>
        </div>
        <div class="db-actions">
            <button class="cx-icon-btn" type="button" aria-pressed={group.pinned} aria-label={group.pinned ? "Unpin group" : "Pin group"} title={group.pinned ? "Unpin group" : "Pin group"} onclick={() => patch({ pinned: !group.pinned })}>
                <Pin class="cx-i" aria-hidden="true" fill={group.pinned ? "currentColor" : "none"} />
            </button>
            <button class="cx-icon-btn nd-danger" type="button" aria-label="Delete group" title="Delete group" onclick={ondelete}>
                <Trash2 class="cx-i" aria-hidden="true" />
            </button>
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={downloadingVpn} aria-busy={downloadingVpn} onclick={downloadVpnConfig}>
                <ShieldCheck class="cx-i" aria-hidden="true" />
                {downloadingVpn ? "Generating" : "Connect VPN"}
            </button>
            <a class="cx-btn cx-btn--primary cx-btn--sm" href="{base}/add">
                <Plus class="cx-i" aria-hidden="true" />
                Add resource
            </a>
        </div>
    </div>
</section>
