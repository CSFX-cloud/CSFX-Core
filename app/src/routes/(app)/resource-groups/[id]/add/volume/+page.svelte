<script lang="ts">
    import { goto } from "$app/navigation";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import CreateVolumeForm from "$lib/components/resource-group/create-volume-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function created() {
        await rg.load();
        goto(base);
    }
</script>

<AddPage type="volume" title="Create a volume">
    <CreateVolumeForm rgId={rg.rgId} onCreated={created} oncancel={() => goto(`${base}/add`)} />
</AddPage>
