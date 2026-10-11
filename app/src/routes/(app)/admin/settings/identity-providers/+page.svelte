<script lang="ts">
    import { onMount } from "svelte";
    import Icon from "@iconify/svelte";
    import { presetFor } from "$lib/components/settings/identity-provider-presets";
    import { toast } from "svelte-sonner";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import CopyIcon from "@lucide/svelte/icons/copy";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteIdentityProvider,
        listIdentityProviders,
        testIdentityProvider,
        type IdentityProvider,
    } from "$lib/api/identity-providers";
    import { listRoles, type Role } from "$lib/api/roles";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import * as Card from "$lib/components/ui/card/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import IdentityProviderDialog from "$lib/components/settings/identity-provider-dialog.svelte";

    let providers = $state<IdentityProvider[]>([]);
    let roles = $state<Role[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let pendingDeleteId = $state<string | null>(null);
    let deleteUsers = $state(false);
    let testingId = $state<string | null>(null);
    let dialog = $state<IdentityProviderDialog | null>(null);

    async function load() {
        if (!auth.token) return;
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
        }
    }

    async function test(provider: IdentityProvider) {
        if (!auth.token) return;
        testingId = provider.id;
        try {
            const result = await testIdentityProvider(auth.token, provider.id);
            toast.success("Discovery succeeded", { description: result.issuer });
        } catch (e) {
            toast.error("Discovery failed", {
                description: e instanceof Error ? e.message : "Provider unreachable",
            });
        } finally {
            testingId = null;
        }
    }

    async function remove(provider: IdentityProvider) {
        if (!auth.token) return;
        try {
            await deleteIdentityProvider(auth.token, provider.id, deleteUsers);
            pendingDeleteId = null;
            await load();
        } catch (e) {
            pendingDeleteId = null;
            error = e instanceof Error ? e.message : "Failed to delete identity provider";
        }
    }

    async function copyRedirectUri(provider: IdentityProvider) {
        await navigator.clipboard.writeText(provider.redirect_uri);
        toast.success("Redirect URI copied");
    }

    function roleName(roleId: string | null): string {
        return roles.find((role) => role.id === roleId)?.name ?? "None";
    }

    onMount(load);
</script>

<IdentityProviderDialog bind:this={dialog} {roles} onSaved={load} />

<div class="flex max-w-3xl flex-col gap-6">
    <div class="flex items-start justify-between gap-4">
        <div>
            <h2 class="text-lg font-light text-foreground/70">Identity providers</h2>
            <p class="text-xs text-muted-foreground">
                Let users sign in with an OpenID Connect provider such as Authentik, Keycloak or Entra ID.
            </p>
        </div>
        <Button onclick={() => dialog?.open(null)}>
            <PlusIcon class="size-4" />
            Add provider
        </Button>
    </div>

    {#if error}
        <p class="px-3 py-2 rounded-md bg-destructive/10 border border-destructive/20 text-sm text-destructive">{error}</p>
    {/if}

    {#if loading}
        <div class="h-24 rounded-lg bg-muted animate-pulse"></div>
    {:else if providers.length === 0}
        <Card.Root>
            <Card.Content class="py-8 text-center text-sm text-muted-foreground">
                No identity providers configured. Add one to enable single sign-on on the login page.
            </Card.Content>
        </Card.Root>
    {:else}
        {#each providers as provider (provider.id)}
            <Card.Root>
                <Card.Header>
                    <div class="flex items-center justify-between gap-3">
                        <div class="min-w-0">
                            <Card.Title class="flex items-center gap-2">
                                {@const preset = presetFor(provider.slug, provider.display_name)}
                                <Icon
                                    icon={preset.icon}
                                    width={18}
                                    height={18}
                                    color={preset.iconColor}
                                    class={provider.enabled ? "" : "opacity-40 grayscale"}
                                />
                                {provider.display_name}
                                <StatusBadge
                                    status={provider.enabled ? "active" : "disabled"}
                                    label={provider.enabled ? "Enabled" : "Disabled"}
                                />
                            </Card.Title>
                            <Card.Description class="truncate font-mono">{provider.issuer_url}</Card.Description>
                        </div>
                        <div class="flex shrink-0 items-center gap-2">
                            <Button size="sm" variant="outline" onclick={() => test(provider)} disabled={testingId === provider.id}>
                                {testingId === provider.id ? "Testing..." : "Test"}
                            </Button>
                            <Button size="sm" variant="outline" onclick={() => dialog?.open(provider)}>Edit</Button>
                            {#if pendingDeleteId === provider.id}
                                <Button size="sm" variant="destructive" onclick={() => remove(provider)}>Confirm</Button>
                                <Button size="sm" variant="ghost" onclick={() => (pendingDeleteId = null)}>Cancel</Button>
                            {:else}
                                <Button
                                    size="sm"
                                    variant="ghost"
                                    onclick={() => {
                                        deleteUsers = false;
                                        pendingDeleteId = provider.id;
                                    }}
                                >
                                    Delete
                                </Button>
                            {/if}
                        </div>
                    </div>
                </Card.Header>
                <Card.Content class="flex flex-col gap-3 text-xs">
                    <div class="flex items-center justify-between gap-2">
                        <span class="text-muted-foreground">Redirect URI</span>
                        <button
                            class="flex min-w-0 items-center gap-1.5 rounded px-1.5 py-0.5 font-mono hover:bg-muted transition-colors"
                            onclick={() => copyRedirectUri(provider)}
                            title="Copy redirect URI"
                        >
                            <span class="truncate">{provider.redirect_uri}</span>
                            <CopyIcon class="size-3 shrink-0" />
                        </button>
                    </div>
                    <div class="flex items-center justify-between gap-2">
                        <span class="text-muted-foreground">Default role</span>
                        <span>{roleName(provider.default_role_id)}</span>
                    </div>
                    <div class="flex items-center justify-between gap-2">
                        <span class="text-muted-foreground">Create users on first sign-in</span>
                        <span>{provider.auto_provision ? "Yes" : "No"}</span>
                    </div>
                    <div class="flex items-center justify-between gap-2">
                        <span class="text-muted-foreground">Users signed in through this provider</span>
                        <span>{provider.user_count}</span>
                    </div>
                    {#if pendingDeleteId === provider.id}
                        <div class="flex items-center justify-between gap-3 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2">
                            <span class="text-sm">
                                {#if provider.user_count > 0}
                                    Also delete {provider.user_count} {provider.user_count === 1 ? "user" : "users"} and their SSH keys
                                {:else}
                                    No users belong to this provider
                                {/if}
                            </span>
                            <Switch bind:checked={deleteUsers} disabled={provider.user_count === 0} />
                        </div>
                    {/if}
                </Card.Content>
            </Card.Root>
        {/each}
    {/if}
</div>
