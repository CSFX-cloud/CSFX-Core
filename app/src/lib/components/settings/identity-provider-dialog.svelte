<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import {
        createIdentityProvider,
        listGroupMappings,
        replaceGroupMappings,
        updateIdentityProvider,
        type GroupMapping,
        type IdentityProvider,
        type IdentityProviderInput,
    } from "$lib/api/identity-providers";
    import type { Role } from "$lib/api/roles";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    const SLUG_PATTERN = /^[a-z0-9-]{1,40}$/;

    let { roles, onSaved }: { roles: Role[]; onSaved: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let editing = $state<IdentityProvider | null>(null);
    let slug = $state("");
    let displayName = $state("");
    let issuerUrl = $state("");
    let clientId = $state("");
    let clientSecret = $state("");
    let scopes = $state("openid profile email");
    let usernameClaim = $state("preferred_username");
    let emailClaim = $state("email");
    let groupsClaim = $state("groups");
    let defaultRoleId = $state("");
    let autoProvision = $state(true);
    let enabled = $state(true);
    let mappings = $state<GroupMapping[]>([]);
    let saving = $state(false);
    let error = $state<string | null>(null);

    const valid = $derived(
        displayName.trim() !== "" &&
            issuerUrl.trim() !== "" &&
            clientId.trim() !== "" &&
            (editing !== null || (SLUG_PATTERN.test(slug) && clientSecret !== "")) &&
            mappings.every((mapping) => mapping.external_group.trim() !== "" && mapping.role_id !== ""),
    );

    function reset(provider: IdentityProvider | null) {
        editing = provider;
        slug = provider?.slug ?? "";
        displayName = provider?.display_name ?? "";
        issuerUrl = provider?.issuer_url ?? "";
        clientId = provider?.client_id ?? "";
        clientSecret = "";
        scopes = provider?.scopes ?? "openid profile email";
        usernameClaim = provider?.username_claim ?? "preferred_username";
        emailClaim = provider?.email_claim ?? "email";
        groupsClaim = provider?.groups_claim ?? "groups";
        defaultRoleId = provider?.default_role_id ?? "";
        autoProvision = provider?.auto_provision ?? true;
        enabled = provider?.enabled ?? true;
        mappings = [];
        error = null;
    }

    export async function open(provider: IdentityProvider | null) {
        reset(provider);
        dialog?.open();
        if (provider && auth.token) {
            try {
                mappings = await listGroupMappings(auth.token, provider.id);
            } catch (e) {
                error = e instanceof Error ? e.message : "Failed to load group mappings";
            }
        }
    }

    function buildInput(): IdentityProviderInput {
        return {
            display_name: displayName.trim(),
            issuer_url: issuerUrl.trim(),
            client_id: clientId.trim(),
            client_secret: clientSecret === "" ? undefined : clientSecret,
            scopes: scopes.trim(),
            username_claim: usernameClaim.trim(),
            email_claim: emailClaim.trim(),
            groups_claim: groupsClaim.trim(),
            default_role_id: defaultRoleId === "" ? null : defaultRoleId,
            auto_provision: autoProvision,
            enabled,
        };
    }

    async function save() {
        if (!auth.token || !valid) return;
        saving = true;
        error = null;
        try {
            const input = buildInput();
            const saved = editing
                ? await updateIdentityProvider(auth.token, editing.id, input)
                : await createIdentityProvider(auth.token, slug, input);
            await replaceGroupMappings(
                auth.token,
                saved.id,
                mappings.map((mapping) => ({ ...mapping, external_group: mapping.external_group.trim() })),
            );
            dialog?.close();
            await onSaved();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to save identity provider";
        } finally {
            saving = false;
        }
    }

    function addMapping() {
        mappings.push({ external_group: "", role_id: roles[0]?.id ?? "", priority: 0 });
    }

    function removeMapping(index: number) {
        mappings.splice(index, 1);
    }
</script>

<ModalDialog
    bind:this={dialog}
    title={editing ? "Edit identity provider" : "Add identity provider"}
    subtitle={editing?.slug}
    width="max-w-2xl"
    onclose={() => (error = null)}
>
    <div class="grid max-h-[65vh] grid-cols-2 gap-3 overflow-y-auto pr-1">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-name">Display name</label>
            <input id="idp-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="Authentik" bind:value={displayName} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-slug">Slug</label>
            <input
                id="idp-slug"
                class="border rounded px-3 py-1.5 text-sm font-mono bg-background disabled:opacity-60"
                placeholder="authentik"
                bind:value={slug}
                disabled={editing !== null}
            />
        </div>
        <div class="col-span-2 flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-issuer">Issuer URL</label>
            <input
                id="idp-issuer"
                class="border rounded px-3 py-1.5 text-sm font-mono bg-background"
                placeholder="https://auth.example.com/application/o/csfx/"
                bind:value={issuerUrl}
            />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-client-id">Client ID</label>
            <input id="idp-client-id" class="border rounded px-3 py-1.5 text-sm font-mono bg-background" bind:value={clientId} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-client-secret">Client secret</label>
            <input
                id="idp-client-secret"
                type="password"
                autocomplete="off"
                class="border rounded px-3 py-1.5 text-sm font-mono bg-background"
                placeholder={editing ? "Unchanged" : ""}
                bind:value={clientSecret}
            />
        </div>
        {#if editing}
            <div class="col-span-2 flex flex-col gap-1">
                <span class="text-xs text-muted-foreground">Redirect URI</span>
                <code class="rounded border bg-muted/40 px-3 py-1.5 text-xs break-all">{editing.redirect_uri}</code>
            </div>
        {/if}
        <div class="col-span-2 flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-scopes">Scopes</label>
            <input id="idp-scopes" class="border rounded px-3 py-1.5 text-sm font-mono bg-background" bind:value={scopes} />
        </div>
        <div class="col-span-2 grid grid-cols-3 gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="idp-username-claim">Username claim</label>
                <input id="idp-username-claim" class="border rounded px-3 py-1.5 text-sm font-mono bg-background" bind:value={usernameClaim} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="idp-email-claim">Email claim</label>
                <input id="idp-email-claim" class="border rounded px-3 py-1.5 text-sm font-mono bg-background" bind:value={emailClaim} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="idp-groups-claim">Groups claim</label>
                <input id="idp-groups-claim" class="border rounded px-3 py-1.5 text-sm font-mono bg-background" bind:value={groupsClaim} />
            </div>
        </div>
        <div class="col-span-2 flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="idp-default-role">Default role (when no group matches)</label>
            <select id="idp-default-role" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={defaultRoleId}>
                <option value="">None, deny sign-in</option>
                {#each roles as role (role.id)}
                    <option value={role.id}>{role.name}</option>
                {/each}
            </select>
        </div>
        <div class="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
            <span class="text-sm">Create users on first sign-in</span>
            <Switch bind:checked={autoProvision} />
        </div>
        <div class="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
            <span class="text-sm">Enabled</span>
            <Switch bind:checked={enabled} />
        </div>

        <div class="col-span-2 mt-2 flex flex-col gap-2">
            <div class="flex items-center justify-between">
                <h3 class="text-xs font-semibold">Group mappings</h3>
                <Button size="sm" variant="outline" onclick={addMapping} disabled={roles.length === 0}>Add mapping</Button>
            </div>
            {#each mappings as mapping, index (index)}
                <div class="grid grid-cols-[1fr_1fr_5rem_auto] items-center gap-2">
                    <input
                        class="border rounded px-3 py-1.5 text-sm font-mono bg-background"
                        placeholder="Group name"
                        aria-label="External group"
                        bind:value={mapping.external_group}
                    />
                    <select class="border rounded px-3 py-1.5 text-sm bg-background" aria-label="Role" bind:value={mapping.role_id}>
                        {#each roles as role (role.id)}
                            <option value={role.id}>{role.name}</option>
                        {/each}
                    </select>
                    <input
                        type="number"
                        class="border rounded px-3 py-1.5 text-sm bg-background"
                        aria-label="Priority"
                        title="Priority, highest wins"
                        bind:value={mapping.priority}
                    />
                    <Button size="sm" variant="ghost" onclick={() => removeMapping(index)}>Remove</Button>
                </div>
            {:else}
                <p class="text-xs text-muted-foreground">
                    No mappings. Members of unmapped groups get the default role.
                </p>
            {/each}
        </div>
    </div>
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex gap-2 justify-end">
        <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
        <Button size="sm" onclick={save} disabled={saving || !valid}>
            {saving ? "Saving..." : "Save"}
        </Button>
    </div>
</ModalDialog>
