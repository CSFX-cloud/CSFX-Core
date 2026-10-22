<script lang="ts">
    import { onMount } from "svelte";
    import { Plus } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteRole, listPermissions, listRoles, type Permission, type Role } from "$lib/api/roles";
    import { pluralize } from "$lib/utils/format";
    import RoleDialog from "$lib/components/settings/role-dialog.svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";

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

<SettingsPage title="Roles" description="Roles bundle permissions. Assign them to users or map them to identity provider groups.">
    {#snippet actions()}
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => dialog?.open(null)}>
            <Plus class="cx-i" aria-hidden="true" />
            Create role
        </button>
    {/snippet}

    {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

    {#if loading}
        <div class="db-skeleton db-skeleton--row" style="height: 96px" aria-hidden="true"></div>
    {:else}
        <section class="cx-panel st-panel" aria-label="Roles">
            <div>
                {#each roles as role (role.id)}
                    <div class="st-role">
                        <div style="min-width: 0">
                            <h3>{role.name}{#if role.is_system_role}<span class="cx-badge">System</span>{/if}</h3>
                            <p>{role.description ?? "No description"} &middot; {pluralize(role.permission_ids.length, "permission")}</p>
                        </div>
                        <div class="db-actions" style="flex-shrink: 0">
                            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => dialog?.open(role)}>
                                {role.is_system_role ? "View" : "Edit"}
                            </button>
                            {#if !role.is_system_role}
                                {#if pendingDeleteId === role.id}
                                    <button class="cx-btn cx-btn--primary cx-btn--sm nd-danger" type="button" onclick={() => remove(role)}>Confirm</button>
                                    <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (pendingDeleteId = null)}>Cancel</button>
                                {:else}
                                    <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (pendingDeleteId = role.id)}>Delete</button>
                                {/if}
                            {/if}
                        </div>
                    </div>
                {/each}
            </div>
        </section>
    {/if}
</SettingsPage>
