<script lang="ts">
    import SideDrawer from "$lib/components/shell/side-drawer.svelte";
    import UserAvatar from "$lib/components/settings/user-avatar.svelte";
    import type { OrganizationRole, OrganizationUser } from "$lib/api/users";

    type Props = {
        user: OrganizationUser | null;
        roles: OrganizationRole[];
        isSelf: boolean;
        busy: boolean;
        onClose: () => void;
        onRoleChange: (roleId: string) => void;
        onDelete: () => void;
    };

    let { user, roles, isSelf, busy, onClose, onRoleChange, onDelete }: Props = $props();

    let confirmingDelete = $state(false);

    const rows = $derived(
        user
            ? [
                  { label: "User ID", value: user.id, mono: true },
                  { label: "Email", value: user.email ?? "\u2014", mono: false },
                  { label: "Avatar email", value: user.gravatar_email ?? "\u2014", mono: false },
                  { label: "Sign-in", value: user.auth_source === "local" ? "Local" : "Single sign-on", mono: false },
                  { label: "Two-factor", value: user.two_factor_enabled ? "On" : "Off", mono: false },
                  { label: "Password change required", value: user.force_password_change ? "Yes" : "No", mono: false },
                  { label: "Joined", value: user.joined_at.slice(0, 10), mono: true },
              ]
            : [],
    );

    $effect(() => {
        if (user) confirmingDelete = false;
    });
</script>

<SideDrawer open={user !== null} title={user?.username ?? ""} subtitle={user?.email ?? user?.gravatar_email ?? "No email"} onclose={onClose}>
    {#snippet header()}
        {#if user}<UserAvatar name={user.username} gravatarEmail={user.gravatar_email} />{/if}
    {/snippet}
    {#if user}
        <dl class="nd-kv">
            {#each rows as row (row.label)}
                <div><dt>{row.label}</dt><dd class:mono={row.mono} style="word-break: break-all">{row.value}</dd></div>
            {/each}
        </dl>

        <div class="cx-field">
            <label class="cx-label" for="drawer-role">Role</label>
            <select class="cx-input" id="drawer-role" value={user.role_id} disabled={busy || isSelf} onchange={(event) => onRoleChange(event.currentTarget.value)}>
                {#each roles as role (role.id)}
                    <option value={role.id}>{role.name}</option>
                {/each}
            </select>
            {#if isSelf}<p class="cx-hint">You cannot change your own role.</p>{/if}
        </div>

        {#if !isSelf}
            <div class="db-actions">
                {#if confirmingDelete}
                    <button class="cx-btn cx-btn--primary cx-btn--sm nd-danger" type="button" disabled={busy} onclick={onDelete}>Confirm delete</button>
                    <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (confirmingDelete = false)}>Cancel</button>
                {:else}
                    <button class="cx-btn cx-btn--secondary cx-btn--sm nd-danger" type="button" onclick={() => (confirmingDelete = true)}>Delete user</button>
                {/if}
            </div>
        {/if}
    {/if}
</SideDrawer>
