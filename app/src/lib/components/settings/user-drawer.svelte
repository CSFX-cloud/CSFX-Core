<script lang="ts">
    import * as Sheet from "$lib/components/ui/sheet/index.js";
    import { Badge } from "$lib/components/ui/badge/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import UserAvatar from "$lib/components/settings/user-avatar.svelte";
    import type { OrganizationRole, OrganizationUser } from "$lib/api/users";

    let {
        user,
        roles,
        isSelf,
        busy,
        onClose,
        onRoleChange,
        onDelete,
    }: {
        user: OrganizationUser | null;
        roles: OrganizationRole[];
        isSelf: boolean;
        busy: boolean;
        onClose: () => void;
        onRoleChange: (roleId: string) => void;
        onDelete: () => void;
    } = $props();

    let confirmingDelete = $state(false);

    $effect(() => {
        if (user) confirmingDelete = false;
    });

    function sourceLabel(source: string): string {
        return source === "local" ? "Local" : "Single sign-on";
    }
</script>

<Sheet.Root open={user !== null} onOpenChange={(open) => !open && onClose()}>
    <Sheet.Content class="sm:max-w-md">
        {#if user}
            <Sheet.Header class="pt-6">
                <div class="flex items-center gap-3">
                    <UserAvatar name={user.username} gravatarEmail={user.gravatar_email} />
                    <div class="min-w-0">
                        <Sheet.Title class="truncate">{user.username}</Sheet.Title>
                        <Sheet.Description class="truncate">{user.email ?? user.gravatar_email ?? "No email"}</Sheet.Description>
                    </div>
                </div>
            </Sheet.Header>

            <div class="flex flex-col gap-3 border-y border-dashed border-border px-4 py-5 text-xs">
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">User ID</span>
                    <span class="font-mono">{user.id}</span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Email</span>
                    <span>{user.email ?? "-"}</span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Avatar email</span>
                    <span>{user.gravatar_email ?? "-"}</span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Sign-in</span>
                    <Badge variant={user.auth_source === "local" ? "secondary" : "pending"}>{sourceLabel(user.auth_source)}</Badge>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Two-factor</span>
                    <Badge variant={user.two_factor_enabled ? "success" : "outline"}>
                        {user.two_factor_enabled ? "On" : "Off"}
                    </Badge>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Password change required</span>
                    <span>{user.force_password_change ? "Yes" : "No"}</span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Joined</span>
                    <span>{user.joined_at.slice(0, 10)}</span>
                </div>
            </div>

            <div class="flex flex-col gap-1 px-4">
                <label class="text-xs text-muted-foreground" for="drawer-role">Role</label>
                <select
                    id="drawer-role"
                    class="border rounded px-3 py-1.5 text-sm bg-background disabled:opacity-60"
                    value={user.role_id}
                    disabled={busy || isSelf}
                    onchange={(event) => onRoleChange(event.currentTarget.value)}
                >
                    {#each roles as role (role.id)}
                        <option value={role.id}>{role.name}</option>
                    {/each}
                </select>
                {#if isSelf}
                    <p class="text-xs text-muted-foreground">You cannot change your own role.</p>
                {/if}
            </div>

            {#if !isSelf}
                <Sheet.Footer class="mt-auto">
                    {#if confirmingDelete}
                        <Button variant="destructive" onclick={onDelete} disabled={busy}>Confirm delete</Button>
                        <Button variant="ghost" onclick={() => (confirmingDelete = false)}>Cancel</Button>
                    {:else}
                        <Button
                            variant="outline"
                            class="border-destructive/40 text-destructive hover:bg-destructive/10"
                            onclick={() => (confirmingDelete = true)}
                        >
                            Delete user
                        </Button>
                    {/if}
                </Sheet.Footer>
            {/if}
        {/if}
    </Sheet.Content>
</Sheet.Root>
