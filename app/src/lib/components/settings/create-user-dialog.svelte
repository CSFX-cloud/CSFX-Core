<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createUser, type OrganizationRole } from "$lib/api/users";
    import PasswordInput from "$lib/components/auth/password-input.svelte";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";
    import Switch from "$lib/components/shell/switch.svelte";

    const MIN_PASSWORD_LENGTH = 8;

    let { roles, onCreated }: { roles: OrganizationRole[]; onCreated: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let username = $state("");
    let email = $state("");
    let password = $state("");
    let roleId = $state("");
    let forcePasswordChange = $state(true);
    let creating = $state(false);
    let error = $state<string | null>(null);

    const valid = $derived(
        username.trim() !== "" && password.length >= MIN_PASSWORD_LENGTH && roleId !== "",
    );

    export function open() {
        username = "";
        email = "";
        password = "";
        roleId = roles.find((role) => role.name === "Viewer")?.id ?? roles[0]?.id ?? "";
        forcePasswordChange = true;
        error = null;
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !valid) return;
        creating = true;
        error = null;
        try {
            await createUser(auth.token, {
                username: username.trim(),
                email: email.trim() === "" ? null : email.trim(),
                password,
                role_id: roleId,
                force_password_change: forcePasswordChange,
            });
            dialog?.close();
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create user";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Create user" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="new-user-name">Username</label>
            <input class="cx-input" id="new-user-name" autocomplete="off" bind:value={username} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="new-user-email">Email <span style="color: var(--ink-muted)">Optional</span></label>
            <input class="cx-input" id="new-user-email" type="email" autocomplete="off" bind:value={email} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="new-user-password">Initial password</label>
            <PasswordInput id="new-user-password" autocomplete="new-password" placeholder="At least {MIN_PASSWORD_LENGTH} characters" bind:value={password} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="new-user-role">Role</label>
            <select class="cx-input" id="new-user-role" bind:value={roleId}>
                {#each roles as role (role.id)}
                    <option value={role.id}>{role.name}</option>
                {/each}
            </select>
        </div>
        <div class="st-note st-split">
            <span>Require password change on first sign-in</span>
            <Switch checked={forcePasswordChange} label="Require password change" onchange={(value) => (forcePasswordChange = value)} />
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !valid} aria-busy={creating}>
                {creating ? "Creating" : "Create"}
            </button>
        </div>
    </form>
</ModalDialog>
