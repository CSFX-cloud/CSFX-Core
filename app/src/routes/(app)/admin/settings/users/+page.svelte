<script lang="ts">
    import { onMount } from "svelte";
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
    import * as Card from "$lib/components/ui/card/index.js";
    import UserAvatar from "$lib/components/settings/user-avatar.svelte";

    let users = $state<OrganizationUser[]>([]);
    let roles = $state<OrganizationRole[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let pendingDeleteId = $state<string | null>(null);
    let savingId = $state<string | null>(null);

    async function load() {
        if (!auth.token) return;
        try {
            [users, roles] = await Promise.all([listUsers(auth.token), listOrganizationRoles(auth.token)]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load users";
        } finally {
            loading = false;
        }
    }

    async function changeRole(user: OrganizationUser, roleId: string) {
        if (!auth.token || roleId === user.role_id) return;
        savingId = user.id;
        try {
            await updateUserRole(auth.token, user.id, roleId);
            await load();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to update role";
        } finally {
            savingId = null;
        }
    }

    async function remove(user: OrganizationUser) {
        if (!auth.token) return;
        try {
            await deleteUser(auth.token, user.id);
            pendingDeleteId = null;
            await load();
        } catch (e) {
            pendingDeleteId = null;
            error = e instanceof Error ? e.message : "Failed to delete user";
        }
    }

    function sourceLabel(source: string): string {
        return source === "local" ? "Local" : "SSO";
    }

    onMount(load);
</script>

<div class="flex max-w-4xl flex-col gap-6">
    <div>
        <h2 class="text-lg font-light text-foreground/70">Users</h2>
        <p class="text-xs text-muted-foreground">
            Everyone who can sign in, including users created through single sign-on.
        </p>
    </div>

    {#if error}
        <p class="px-3 py-2 rounded-md bg-destructive/10 border border-destructive/20 text-sm text-destructive">{error}</p>
    {/if}

    {#if loading}
        <div class="h-24 rounded-lg bg-muted animate-pulse"></div>
    {:else}
        <Card.Root>
            <Card.Content class="p-0">
                <table class="w-full text-sm">
                    <thead>
                        <tr class="border-b text-left text-xs text-muted-foreground">
                            <th class="px-4 py-2.5 font-medium">User</th>
                            <th class="px-4 py-2.5 font-medium">Role</th>
                            <th class="px-4 py-2.5 font-medium">Sign-in</th>
                            <th class="px-4 py-2.5 font-medium">2FA</th>
                            <th class="px-4 py-2.5 font-medium">Joined</th>
                            <th class="px-4 py-2.5"></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each users as user (user.id)}
                            <tr class="border-b last:border-0">
                                <td class="px-4 py-3">
                                    <div class="flex items-center gap-3">
                                        <UserAvatar name={user.username} gravatarEmail={user.gravatar_email} />
                                        <div class="min-w-0">
                                            <div class="truncate font-medium">{user.username}</div>
                                            <div class="truncate text-xs text-muted-foreground">{user.email ?? "No email"}</div>
                                        </div>
                                    </div>
                                </td>
                                <td class="px-4 py-3">
                                    <select
                                        class="border rounded px-2 py-1 text-xs bg-background disabled:opacity-60"
                                        value={user.role_id}
                                        disabled={savingId === user.id || user.id === auth.user?.id}
                                        onchange={(event) => changeRole(user, event.currentTarget.value)}
                                        aria-label="Role of {user.username}"
                                    >
                                        {#each roles as role (role.id)}
                                            <option value={role.id}>{role.name}</option>
                                        {/each}
                                    </select>
                                </td>
                                <td class="px-4 py-3">
                                    <Badge variant={user.auth_source === "local" ? "secondary" : "pending"}>
                                        {sourceLabel(user.auth_source)}
                                    </Badge>
                                </td>
                                <td class="px-4 py-3">
                                    <Badge variant={user.two_factor_enabled ? "success" : "outline"}>
                                        {user.two_factor_enabled ? "On" : "Off"}
                                    </Badge>
                                </td>
                                <td class="px-4 py-3 text-xs text-muted-foreground">{user.joined_at.slice(0, 10)}</td>
                                <td class="px-4 py-3 text-right">
                                    {#if user.id !== auth.user?.id}
                                        {#if pendingDeleteId === user.id}
                                            <Button size="sm" variant="destructive" onclick={() => remove(user)}>Confirm</Button>
                                            <Button size="sm" variant="ghost" onclick={() => (pendingDeleteId = null)}>Cancel</Button>
                                        {:else}
                                            <Button size="sm" variant="ghost" onclick={() => (pendingDeleteId = user.id)}>Delete</Button>
                                        {/if}
                                    {/if}
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </Card.Content>
        </Card.Root>
    {/if}
</div>
