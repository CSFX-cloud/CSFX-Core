<script lang="ts">
    import { onMount } from "svelte";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import UsersIcon from "@lucide/svelte/icons/users";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteUser,
        listOrganizationRoles,
        listUsers,
        updateUserRole,
        type OrganizationRole,
        type OrganizationUser,
    } from "$lib/api/users";
    import { Badge } from "$lib/components/ui/badge/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import CreateUserDialog from "$lib/components/settings/create-user-dialog.svelte";
    import UserAvatar from "$lib/components/settings/user-avatar.svelte";
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

<div class="flex min-w-0 flex-1 flex-col gap-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-xl font-semibold tracking-tight">Users</h1>
            <p class="text-sm text-muted-foreground mt-0.5">Manage everyone who can sign in</p>
        </div>
        <Button size="sm" onclick={() => createDialog?.open()}>
            <PlusIcon class="size-3.5" />
            Create user
        </Button>
    </div>

    <div class="border rounded-lg overflow-hidden">
        <div class="flex items-center justify-between px-4 py-3 border-b bg-muted/30">
            <div class="flex items-center gap-2">
                <UsersIcon class="size-4 text-muted-foreground" />
                <span class="text-sm font-semibold">User Summary</span>
            </div>
            <Button variant="ghost" size="icon-sm" onclick={load} disabled={refreshing} aria-label="Refresh">
                <RefreshCwIcon class="size-4 {refreshing ? 'animate-spin' : ''}" />
            </Button>
        </div>
        <table class="w-full text-sm">
            <thead class="bg-muted/50">
                <tr>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">ID</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">User</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Email</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Role</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Sign-in</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">2FA</th>
                    <th class="text-left px-4 py-3 font-medium text-muted-foreground">Joined</th>
                </tr>
            </thead>
            <tbody>
                {#if loading}
                    <tr>
                        <td colspan="7" class="px-4 py-8 text-center text-muted-foreground">Loading...</td>
                    </tr>
                {:else if error}
                    <tr>
                        <td colspan="7" class="px-4 py-8 text-center text-destructive">{error}</td>
                    </tr>
                {:else if users.length === 0}
                    <tr>
                        <td colspan="7" class="px-4 py-8 text-center text-muted-foreground">No users</td>
                    </tr>
                {:else}
                    {#each users as user (user.id)}
                        <tr
                            class="border-t hover:bg-muted/30 transition-colors cursor-pointer"
                            onclick={() => (selectedId = user.id)}
                        >
                            <td class="px-4 py-3 font-mono text-xs text-muted-foreground">{user.id.slice(0, 8)}</td>
                            <td class="px-4 py-3">
                                <div class="flex items-center gap-3">
                                    <UserAvatar name={user.username} gravatarEmail={user.gravatar_email} />
                                    <span class="font-medium">{user.username}</span>
                                </div>
                            </td>
                            <td class="px-4 py-3 text-muted-foreground">{user.email ?? user.gravatar_email ?? "-"}</td>
                            <td class="px-4 py-3">{user.role_name}</td>
                            <td class="px-4 py-3">
                                <Badge variant={user.auth_source === "local" ? "secondary" : "pending"}>
                                    {user.auth_source === "local" ? "Local" : "SSO"}
                                </Badge>
                            </td>
                            <td class="px-4 py-3">
                                <Badge variant={user.two_factor_enabled ? "success" : "outline"}>
                                    {user.two_factor_enabled ? "On" : "Off"}
                                </Badge>
                            </td>
                            <td class="px-4 py-3 text-muted-foreground">{user.joined_at.slice(0, 10)}</td>
                        </tr>
                    {/each}
                {/if}
            </tbody>
        </table>
    </div>
</div>
