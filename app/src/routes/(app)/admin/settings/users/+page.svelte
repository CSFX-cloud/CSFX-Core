<script lang="ts">
    import { onMount } from "svelte";
    import { Plus, RefreshCw, Users } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteUser,
        listOrganizationRoles,
        listUsers,
        updateUserRole,
        type OrganizationRole,
        type OrganizationUser,
    } from "$lib/api/users";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import CreateUserDialog from "$lib/components/settings/create-user-dialog.svelte";
    import UserAvatar from "$lib/components/settings/user-avatar.svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import UserDrawer from "$lib/components/settings/user-drawer.svelte";

    let users = $state<OrganizationUser[]>([]);
    let roles = $state<OrganizationRole[]>([]);
    let loading = $state(true);
    let refreshing = $state(false);
    let busy = $state(false);
    let error = $state<string | null>(null);
    let selectedId = $state<string | null>(null);
    let createDialog = $state<CreateUserDialog | null>(null);

    const selected = $derived(users.find((user) => user.id === selectedId) ?? null);

    async function load() {
        if (!auth.token) return;
        refreshing = true;
        try {
            [users, roles] = await Promise.all([listUsers(auth.token), listOrganizationRoles(auth.token)]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load users";
        } finally {
            loading = false;
            refreshing = false;
        }
    }

    async function changeRole(roleId: string) {
        if (!auth.token || !selected || roleId === selected.role_id) return;
        busy = true;
        try {
            await updateUserRole(auth.token, selected.id, roleId);
            await load();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to update role";
        } finally {
            busy = false;
        }
    }

    async function removeSelected() {
        if (!auth.token || !selected) return;
        busy = true;
        try {
            await deleteUser(auth.token, selected.id);
            selectedId = null;
            await load();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete user";
        } finally {
            busy = false;
        }
    }

    onMount(load);
</script>

<CreateUserDialog bind:this={createDialog} {roles} onCreated={load} />
<UserDrawer
    user={selected}
    {roles}
    isSelf={selected?.id === auth.user?.id}
    {busy}
    onClose={() => (selectedId = null)}
    onRoleChange={changeRole}
    onDelete={removeSelected}
/>

<SettingsPage title="Users" description="Manage everyone who can sign in.">
    {#snippet actions()}
        <button class="cx-icon-btn" type="button" style="width: 36px; height: 36px" aria-label="Refresh" disabled={refreshing} onclick={load}>
            <RefreshCw class="cx-i {refreshing ? 'db-spin' : ''}" aria-hidden="true" />
        </button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => createDialog?.open()}>
            <Plus class="cx-i" aria-hidden="true" />
            Create user
        </button>
    {/snippet}

    {#if error}
        <p class="cx-hint cx-hint--error">{error}</p>
    {:else if loading}
        <div class="db-skeleton-list" aria-hidden="true">
            {#each { length: 3 } as _, index (index)}<span class="db-skeleton db-skeleton--row"></span>{/each}
        </div>
    {:else if users.length === 0}
        <EmptyState icon={Users} label="No users" />
    {:else}
        <section class="cx-panel" aria-label="Users">
            <div class="db-scroll">
                <table class="cx-table">
                    <thead><tr><th>User</th><th>Email</th><th>Role</th><th>Sign-in</th><th>2FA</th><th>Joined</th></tr></thead>
                    <tbody>
                        {#each users as user (user.id)}
                            <tr class="nl-row">
                                <td>
                                    <div class="rs-name">
                                        <UserAvatar name={user.username} gravatarEmail={user.gravatar_email} />
                                        <button class="rs-link" type="button" onclick={() => (selectedId = user.id)}>{user.username}</button>
                                    </div>
                                </td>
                                <td class="cx-soft">{user.email ?? user.gravatar_email ?? "\u2014"}</td>
                                <td class="cx-soft">{user.role_name}</td>
                                <td><span class="cx-badge">{user.auth_source === "local" ? "Local" : "SSO"}</span></td>
                                <td><span class="cx-badge {user.two_factor_enabled ? 'cx-badge--success' : ''}">{user.two_factor_enabled ? "On" : "Off"}</span></td>
                                <td class="cx-num">{user.joined_at.slice(0, 10)}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </section>
    {/if}
</SettingsPage>
