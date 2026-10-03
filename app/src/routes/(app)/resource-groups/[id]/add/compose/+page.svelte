<script lang="ts">
    import { goto } from "$app/navigation";
    import ComposeEditor from "$lib/components/resource-group/compose-editor.svelte";
    import RgTopbar from "$lib/components/resource-group/rg-topbar.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();

    async function done() {
        await rg.load();
        goto(`/resource-groups/${rg.rgId}`);
    }
</script>

<RgTopbar backHref="/resource-groups/{rg.rgId}/add" backLabel="Back to add resource" />
<div class="flex-1 overflow-y-auto">
    <ComposeEditor rgId={rg.rgId} onDone={done} />
</div>
