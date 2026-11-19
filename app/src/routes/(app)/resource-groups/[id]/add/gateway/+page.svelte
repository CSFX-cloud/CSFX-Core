<script lang="ts">
    import { goto } from "$app/navigation";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import CreateGatewayForm from "$lib/components/resource-group/create-gateway-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function created(gatewayId: string) {
        await rg.load();
        goto(`${base}/gateway/${gatewayId}`);
    }
</script>

<AddPage type="api-gateway" title="Create an API gateway">
    <CreateGatewayForm rgId={rg.rgId} onCreated={created} oncancel={() => goto(`${base}/add`)} />
</AddPage>
