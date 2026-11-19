<script lang="ts">
    import { goto } from "$app/navigation";
    import { createVault, type Vault } from "$lib/api/vaults";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import CreateNamedForm from "$lib/components/resource-group/create-named-form.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    async function created(vault: Vault) {
        await rg.load();
        goto(`${base}/vault/${vault.id}`);
    }
</script>

<AddPage type="key-vault" title="Create a key vault">
    <CreateNamedForm
        hint="Lowercase letters, digits and hyphens. Secrets stay encrypted and resources read them at start."
        placeholder="kv-prod"
        inputId="kv-name"
        failure="Failed to create vault"
        create={(name) => createVault(rg.rgId, name)}
        onCreated={created}
        oncancel={() => goto(`${base}/add`)}
    />
</AddPage>
