<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import {
        updateResourceGroup,
        uploadResourceGroupIconImage,
        deleteResourceGroupIconImage,
        resourceGroupIconImageUrl,
        type ResourceGroup,
    } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import IconPicker from "$lib/components/icon-picker.svelte";
    import ModalDialog from "./modal-dialog.svelte";

    let { group, onUpdated }: { group: ResourceGroup | null; onUpdated: (group: ResourceGroup) => void } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let icon = $state("mdi:cube-outline");
    let color = $state("#6366f1");
    let saving = $state(false);
    let error = $state<string | null>(null);
    let uploadingImage = $state(false);
    let imageError = $state<string | null>(null);

    export function open(target: ResourceGroup | null) {
        if (!target) return;
        icon = target.icon;
        color = target.color;
        error = null;
        dialog?.open();
    }

    async function save() {
        if (!auth.token || !group) return;
        saving = true;
        error = null;
        try {
            onUpdated(await updateResourceGroup(auth.token, group.id, { icon, color }));
            dialog?.close();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to update appearance";
        } finally {
            saving = false;
        }
    }

    async function uploadImage(file: File) {
        if (!auth.token || !group) return;
        uploadingImage = true;
        imageError = null;
        try {
            onUpdated(await uploadResourceGroupIconImage(auth.token, group.id, file));
        } catch (e) {
            imageError = e instanceof Error ? e.message : "Failed to upload icon image";
        } finally {
            uploadingImage = false;
        }
    }

    async function removeImage() {
        if (!auth.token || !group) return;
        imageError = null;
        try {
            onUpdated(await deleteResourceGroupIconImage(auth.token, group.id));
        } catch (e) {
            imageError = e instanceof Error ? e.message : "Failed to remove icon image";
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Edit Appearance" onclose={() => (error = null)}>
    <IconPicker
        bind:icon
        bind:color
        imageUrl={group?.has_icon_image ? `${resourceGroupIconImageUrl(group.id)}?t=${group.updated_at}` : null}
        {uploadingImage}
        {imageError}
        onUploadImage={uploadImage}
        onRemoveImage={removeImage}
    />
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex gap-2 justify-end">
        <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
        <Button size="sm" onclick={save} disabled={saving}>
            {saving ? "Saving..." : "Save"}
        </Button>
    </div>
</ModalDialog>
