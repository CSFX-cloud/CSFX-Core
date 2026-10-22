<script lang="ts">
    import { onMount } from "svelte";
    import Icon from "@iconify/svelte";
    import { toast } from "svelte-sonner";
    import { KeyRound, Plus, RefreshCw } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteIdentityProvider,
        listIdentityProviders,
        testIdentityProvider,
        type IdentityProvider,
    } from "$lib/api/identity-providers";
    import { listRoles, type Role } from "$lib/api/roles";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import IdentityProviderDialog from "$lib/components/settings/identity-provider-dialog.svelte";
    import { presetFor } from "$lib/components/settings/identity-provider-presets";
    import ProviderDrawer from "$lib/components/settings/provider-drawer.svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";

    let providers = $state<IdentityProvider[]>([]);
    let roles = $state<Role[]>([]);
    let loading = $state(true);
    let refreshing = $state(false);
    let error = $state<string | null>(null);
    let selectedId = $state<string | null>(null);
    let testing = $state(false);
    let dialog = $state<IdentityProviderDialog | null>(null);

    const selected = $derived(providers.find((provider) => provider.id === selectedId) ?? null);

    async function load() {
        if (!auth.token) return;
        refreshing = true;
        try {
            [providers, roles] = await Promise.all([
                listIdentityProviders(auth.token),
                listRoles(auth.token),
            ]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load identity providers";
        } finally {
            loading = false;
            refreshing = false;
        }
    }

    async function testSelected() {
        if (!auth.token || !selected) return;
        testing = true;
        try {
            const result = await testIdentityProvider(auth.token, selected.id);
            toast.success("Discovery succeeded", { description: result.issuer });
        } catch (e) {
            toast.error("Discovery failed", {
                description: e instanceof Error ? e.message : "Provider unreachable",
            });
        } finally {
            testing = false;
        }
    }

    async function removeSelected(deleteUsers: boolean) {
        if (!auth.token || !selected) return;
        try {
            await deleteIdentityProvider(auth.token, selected.id, deleteUsers);
            selectedId = null;
            await load();
        } catch (e) {
            toast.error("Delete failed", {
                description: e instanceof Error ? e.message : "Failed to delete identity provider",
            });
        }
    }

    function editSelected() {
        if (!selected) return;
        const provider = selected;
        selectedId = null;
        dialog?.open(provider);
    }

    function roleName(roleId: string | null): string {
        return roles.find((role) => role.id === roleId)?.name ?? "None";
    }

    onMount(load);
</script>

<IdentityProviderDialog bind:this={dialog} {roles} onSaved={load} />
<ProviderDrawer
    provider={selected}
    roleName={roleName(selected?.default_role_id ?? null)}
    {testing}
    onClose={() => (selectedId = null)}
    onTest={testSelected}
    onEdit={editSelected}
    onDelete={removeSelected}
/>

<SettingsPage title="Identity providers" description="Single sign-on with Authentik, Keycloak, Entra ID and other OpenID Connect providers.">
    {#snippet actions()}
        <button class="cx-icon-btn" type="button" style="width: 36px; height: 36px" aria-label="Refresh" disabled={refreshing} onclick={load}>
            <RefreshCw class="cx-i {refreshing ? 'db-spin' : ''}" aria-hidden="true" />
        </button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => dialog?.open(null)}>
            <Plus class="cx-i" aria-hidden="true" />
            Add provider
        </button>
    {/snippet}

    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: 3 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if providers.length === 0}
        <EmptyState icon={KeyRound} label="No identity providers yet. Add one to enable single sign-on on the login page." />
    {:else}
        <section class="cx-panel" aria-label="Identity providers">
            <div class="db-scroll">
                <table class="cx-table">
                    <thead><tr><th>Provider</th><th>Issuer</th><th>Status</th><th>Default role</th><th>Sign-up</th><th>Users</th></tr></thead>
                    <tbody>
                        {#each providers as provider (provider.id)}
                            {@const preset = presetFor(provider.slug, provider.display_name)}
                            <tr class="nl-row">
                                <td>
                                    <div class="rs-name">
                                        <span class="svc-tile" style="background: var(--bg-200)">
                                            <Icon icon={preset.icon} width={18} height={18} color={preset.iconColor} class={provider.enabled ? "" : "opacity-40 grayscale"} />
                                        </span>
                                        <span class="rs-text">
                                            <button class="rs-link" type="button" onclick={() => (selectedId = provider.id)}>{provider.display_name}</button>
                                            <span>{provider.slug}</span>
                                        </span>
                                    </div>
                                </td>
                                <td class="cx-num">{provider.issuer_url}</td>
                                <td><span class="cx-badge {provider.enabled ? 'cx-badge--success' : ''}">{provider.enabled ? "Enabled" : "Disabled"}</span></td>
                                <td class="cx-soft">{roleName(provider.default_role_id)}</td>
                                <td class="cx-soft">{provider.auto_provision ? "Automatic" : "Manual"}</td>
                                <td class="cx-num">{provider.user_count}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </section>
    {/if}
</SettingsPage>
