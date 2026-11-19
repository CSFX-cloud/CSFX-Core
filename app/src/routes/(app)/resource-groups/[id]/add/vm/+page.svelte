<script lang="ts">
    import { goto } from "$app/navigation";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import DeployVmForm from "$lib/components/resource-group/deploy-vm-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function deployed() {
        await rg.load();
        goto(base);
    }
</script>

<AddPage type="vm" title="Deploy a virtual machine">
    <DeployVmForm rgId={rg.rgId} onDeployed={deployed} oncancel={() => goto(`${base}/add`)} />
</AddPage>
