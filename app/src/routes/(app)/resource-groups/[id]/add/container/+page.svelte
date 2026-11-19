<script lang="ts">
    import { goto } from "$app/navigation";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import DeployContainerForm from "$lib/components/resource-group/deploy-container-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function deployed() {
        await rg.load();
        goto(base);
    }
</script>

<AddPage type="docker-container" title="Deploy a container">
    <DeployContainerForm rgId={rg.rgId} volumes={rg.volumes} onDeployed={deployed} oncancel={() => goto(`${base}/add`)} />
</AddPage>
