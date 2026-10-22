<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createRole, updateRole, type Permission, type Role } from "$lib/api/roles";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    let { permissions, onSaved }: { permissions: Permission[]; onSaved: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let editing = $state<Role | null>(null);
    let name = $state("");
    let description = $state("");
    let selected = $state<Set<string>>(new Set());
    let saving = $state(false);
    let error = $state<string | null>(null);

    const permissionsByResource = $derived(
        Object.entries(
            Object.groupBy(permissions, (permission) => permission.resource),
        ) as [string, Permission[]][],
    );

    export function open(role: Role | null) {
        editing = role;
        name = role?.name ?? "";
        description = role?.description ?? "";
        selected = new Set(role?.permission_ids ?? []);
        error = null;
        dialog?.open();
    }

    function toggle(permissionId: string) {
        const next = new Set(selected);
        if (next.has(permissionId)) {
            next.delete(permissionId);
        } else {
            next.add(permissionId);
        }
        selected = next;
    }

    async function save() {
        if (!auth.token || name.trim() === "") return;
        saving = true;
        error = null;
        const input = {
            name: name.trim(),
            description: description.trim() === "" ? null : description.trim(),
            permission_ids: [...selected],
        };
        try {
            if (editing) {
                await updateRole(auth.token, editing.id, input);
            } else {
                await createRole(auth.token, input);
            }
            dialog?.close();
            await onSaved();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to save role";
        } finally {
            saving = false;
        }
    }
</script>

<ModalDialog
    bind:this={dialog}
    title={editing ? (editing.is_system_role ? "Role permissions" : "Edit role") : "Create role"}
    subtitle={editing?.name}
    size="xl"
    onclose={() => (error = null)}
>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            save();
        }}
    >
        <div class="md__fields role-scroll">
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                <div class="cx-field">
                    <label class="cx-label" for="role-name">Name</label>
                    <input class="cx-input" id="role-name" bind:value={name} disabled={editing?.is_system_role} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="role-description">Description</label>
                    <input class="cx-input" id="role-description" bind:value={description} disabled={editing?.is_system_role} />
                </div>
            </div>
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                {#each permissionsByResource as [resource, items] (resource)}
                    <div class="st-note" style="display: grid; gap: 8px">
                        <h3 class="cx-panel__title">{resource}</h3>
                        {#each items as permission (permission.id)}
                            <label class="cx-check" title={permission.description ?? ""} style="font-size: 13px">
                                <input type="checkbox" checked={selected.has(permission.id)} onchange={() => toggle(permission.id)} disabled={editing?.is_system_role} />
                                {permission.action}
                            </label>
                        {/each}
                    </div>
                {/each}
            </div>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>{editing?.is_system_role ? "Close" : "Cancel"}</button>
            {#if !editing?.is_system_role}
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving || name.trim() === ""} aria-busy={saving}>
                    {saving ? "Saving" : "Save"}
                </button>
            {/if}
        </div>
    </form>
</ModalDialog>
