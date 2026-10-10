<script lang="ts">
    import { onMount } from "svelte";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteRole, listPermissions, listRoles, type Permission, type Role } from "$lib/api/roles";
    import { Badge } from "$lib/components/ui/badge/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import * as Card from "$lib/components/ui/card/index.js";
    import RoleDialog from "$lib/components/settings/role-dialog.svelte";

    let roles = $state<Role[]>([]);
    let permissions = $state<Permission[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let pendingDeleteId = $state<string | null>(null);
    let dialog = $state<RoleDialog | null>(null);

    async function load() {
        if (!auth.token) return;
        try {
            [roles, permissions] = await Promise.all([
                listRoles(auth.token),
                listPermissions(auth.token),
            ]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load roles";
        } finally {
            loading = false;
        }
    }

    async function remove(role: Role) {
        if (!auth.token) return;
        try {
            await deleteRole(auth.token, role.id);
            pendingDeleteId = null;
            await load();
        } catch (e) {
            pendingDeleteId = null;
            error = e instanceof Error ? e.message : "Failed to delete role";
        }
    }

    onMount(load);
</script>

<RoleDialog bind:this={dialog} {permissions} onSaved={load} />

<div class="flex max-w-3xl flex-col gap-6">
    <div class="flex items-start justify-between gap-4">
        <div>
            <h2 class="text-lg font-light text-foreground/70">Roles</h2>
            <p class="text-xs text-muted-foreground">
                Roles bundle permissions. Assign them to users or map them to identity provider groups.
            </p>
        </div>
        <Button onclick={() => dialog?.open(null)}>
            <PlusIcon class="size-4" />
            Create role
        </Button>
    </div>

    {#if error}
        <p class="px-3 py-2 rounded-md bg-destructive/10 border border-destructive/20 text-sm text-destructive">{error}</p>
    {/if}

    {#if loading}
        <div class="h-24 rounded-lg bg-muted animate-pulse"></div>
    {:else}
        {#each roles as role (role.id)}
            <Card.Root>
                <Card.Header>
                    <div class="flex items-center justify-between gap-3">
                        <div class="min-w-0">
                            <Card.Title class="flex items-center gap-2">
                                {role.name}
                                {#if role.is_system_role}
                                    <Badge variant="secondary">System</Badge>
                                {/if}
                            </Card.Title>
                            <Card.Description>
                                {role.description ?? "No description"} · {role.permission_ids.length} permissions
                            </Card.Description>
                        </div>
                        <div class="flex shrink-0 items-center gap-2">
                            <Button size="sm" variant="outline" onclick={() => dialog?.open(role)}>
                                {role.is_system_role ? "View" : "Edit"}
                            </Button>
                            {#if !role.is_system_role}
                                {#if pendingDeleteId === role.id}
                                    <Button size="sm" variant="destructive" onclick={() => remove(role)}>Confirm</Button>
                                    <Button size="sm" variant="ghost" onclick={() => (pendingDeleteId = null)}>Cancel</Button>
                                {:else}
                                    <Button size="sm" variant="ghost" onclick={() => (pendingDeleteId = role.id)}>Delete</Button>
                                {/if}
                            {/if}
                        </div>
                    </div>
                </Card.Header>
            </Card.Root>
        {/each}
    {/if}
</div>
