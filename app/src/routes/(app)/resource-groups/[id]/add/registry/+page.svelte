<script lang="ts">
    import { goto } from "$app/navigation";
    import { createRegistry, type Registry } from "$lib/api/registries";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import CreateNamedForm from "$lib/components/resource-group/create-named-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function created(registry: Registry) {
        await rg.load();
        goto(`${base}/registry/${registry.id}`);
    }
</script>

<AddPage type="container-registry" title="Create a container registry">
    <CreateNamedForm
        hint="Lowercase letters, digits and hyphens. Part of every image path. Push images here and deploy them in this resource group."
        placeholder="main"
        inputId="cr-name"
        failure="Failed to create registry"
        create={(name) => createRegistry(rg.rgId, name)}
        onCreated={created}
        oncancel={() => goto(`${base}/add`)}
    />
</AddPage>
