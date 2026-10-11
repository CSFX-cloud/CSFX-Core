<script lang="ts">
    import { onMount } from "svelte";
    import Icon from "@iconify/svelte";
    import { toast } from "svelte-sonner";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import KeyRoundIcon from "@lucide/svelte/icons/key-round";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteIdentityProvider,
        listIdentityProviders,
        testIdentityProvider,
        type IdentityProvider,
    } from "$lib/api/identity-providers";
    import { listRoles, type Role } from "$lib/api/roles";
    import { Button } from "$lib/components/ui/button/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import IdentityProviderDialog from "$lib/components/settings/identity-provider-dialog.svelte";
    import { presetFor } from "$lib/components/settings/identity-provider-presets";
    import ProviderDrawer from "$lib/components/settings/provider-drawer.svelte";

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

<div class="flex min-w-0 flex-1 flex-col gap-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-xl font-semibold tracking-tight">Identity providers</h1>
            <p class="text-sm text-muted-foreground mt-0.5">
                Single sign-on with Authentik, Keycloak, Entra ID and other OpenID Connect providers
            </p>
        </div>
        <Button size="sm" onclick={() => dialog?.open(null)}>
            <PlusIcon class="size-3.5" />
            Add provider
        </Button>
    </div>

    <div class="border rounded-lg overflow-hidden">
        <div class="flex items-center justify-between px-4 py-3 border-b bg-muted/30">
            <div class="flex items-center gap-2">
                <KeyRoundIcon class="size-4 text-muted-foreground" />
                <span class="text-sm font-semibold">Provider Summary</span>
            </div>
            <Button variant="ghost" size="icon-sm" onclick={load} disabled={refreshing} aria-label="Refresh">
                <RefreshCwIcon class="size-4 {refreshing ? 'animate-spin' : ''}" />
            </Button>
        </div>
        <table class="w-full text-sm">
            <thead class="bg-muted/50">
                <tr>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Provider</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Issuer</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Status</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Default role</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Sign-up</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Users</th>
                </tr>
            </thead>
            <tbody>
                {#if loading}
                    <tr>
                        <td colspan="6" class="px-4 py-8 text-center text-muted-foreground">Loading...</td>
                    </tr>
                {:else if error}
                    <tr>
                        <td colspan="6" class="px-4 py-8 text-center text-destructive">{error}</td>
                    </tr>
                {:else if providers.length === 0}
                    <tr>
                        <td colspan="6" class="px-4 py-8 text-center text-muted-foreground">
                            No identity providers configured. Add one to enable single sign-on on the login page.
                        </td>
                    </tr>
                {:else}
                    {#each providers as provider (provider.id)}
                        {@const preset = presetFor(provider.slug, provider.display_name)}
                        <tr
                            class="border-t hover:bg-muted/30 transition-colors cursor-pointer"
                            onclick={() => (selectedId = provider.id)}
                        >
                            <td class="px-4 py-3">
                                <div class="flex items-center gap-3">
                                    <Icon
                                        icon={preset.icon}
                                        width={20}
                                        height={20}
                                        color={preset.iconColor}
                                        class={provider.enabled ? "" : "opacity-40 grayscale"}
                                    />
                                    <div class="min-w-0">
                                        <div class="font-medium">{provider.display_name}</div>
                                        <div class="font-mono text-xs text-muted-foreground">{provider.slug}</div>
                                    </div>
                                </div>
                            </td>
                            <td class="px-4 py-3 font-mono text-xs text-muted-foreground">{provider.issuer_url}</td>
                            <td class="px-4 py-3">
                                <StatusBadge
                                    status={provider.enabled ? "active" : "disabled"}
                                    label={provider.enabled ? "Enabled" : "Disabled"}
                                />
                            </td>
                            <td class="px-4 py-3">{roleName(provider.default_role_id)}</td>
                            <td class="px-4 py-3 text-muted-foreground">
                                {provider.auto_provision ? "Automatic" : "Manual"}
                            </td>
                            <td class="px-4 py-3 text-muted-foreground tabular-nums">{provider.user_count}</td>
                        </tr>
                    {/each}
                {/if}
            </tbody>
        </table>
    </div>
</div>
