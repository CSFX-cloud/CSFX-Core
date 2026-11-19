<script lang="ts">
    import { goto } from "$app/navigation";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import CreateBucketForm from "$lib/components/resource-group/create-bucket-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function created() {
        await rg.load();
        goto(base);
    }
</script>

<AddPage type="bucket" title="Create a bucket">
    <CreateBucketForm rgId={rg.rgId} onCreated={created} oncancel={() => goto(`${base}/add`)} />
</AddPage>
