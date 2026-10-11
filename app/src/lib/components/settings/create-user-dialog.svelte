<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createUser, type OrganizationRole } from "$lib/api/users";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

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
    <div class="grid grid-cols-1 gap-3">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="new-user-name">Username</label>
            <input id="new-user-name" class="border rounded px-3 py-1.5 text-sm bg-background" autocomplete="off" bind:value={username} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="new-user-email">Email (optional)</label>
            <input id="new-user-email" type="email" class="border rounded px-3 py-1.5 text-sm bg-background" autocomplete="off" bind:value={email} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="new-user-password">Initial password</label>
            <input
                id="new-user-password"
                type="password"
                class="border rounded px-3 py-1.5 text-sm bg-background"
                autocomplete="new-password"
                placeholder="At least {MIN_PASSWORD_LENGTH} characters"
                bind:value={password}
            />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="new-user-role">Role</label>
            <select id="new-user-role" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={roleId}>
                {#each roles as role (role.id)}
                    <option value={role.id}>{role.name}</option>
                {/each}
            </select>
        </div>
        <div class="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
            <span class="text-sm">Require password change on first sign-in</span>
            <Switch bind:checked={forcePasswordChange} />
        </div>
    </div>
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex gap-2 justify-end">
        <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
        <Button size="sm" onclick={submit} disabled={creating || !valid}>
            {creating ? "Creating..." : "Create"}
        </Button>
    </div>
</ModalDialog>
