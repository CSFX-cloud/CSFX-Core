<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createRole, updateRole, type Permission, type Role } from "$lib/api/roles";
    import { Button } from "$lib/components/ui/button/index.js";
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
    width="max-w-2xl"
    onclose={() => (error = null)}
>
    <div class="flex max-h-[65vh] flex-col gap-3 overflow-y-auto pr-1">
        <div class="grid grid-cols-2 gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="role-name">Name</label>
                <input
                    id="role-name"
                    class="border rounded px-3 py-1.5 text-sm bg-background disabled:opacity-60"
                    bind:value={name}
                    disabled={editing?.is_system_role}
                />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="role-description">Description</label>
                <input
                    id="role-description"
                    class="border rounded px-3 py-1.5 text-sm bg-background disabled:opacity-60"
                    bind:value={description}
                    disabled={editing?.is_system_role}
                />
            </div>
        </div>
        <div class="grid grid-cols-2 gap-x-4 gap-y-3">
            {#each permissionsByResource as [resource, items] (resource)}
                <div class="flex flex-col gap-1.5 rounded-md border p-3">
                    <h3 class="text-xs font-semibold">{resource}</h3>
                    {#each items as permission (permission.id)}
                        <label class="flex items-center gap-2 text-xs" title={permission.description ?? ""}>
                            <input
                                type="checkbox"
                                checked={selected.has(permission.id)}
                                onchange={() => toggle(permission.id)}
                                disabled={editing?.is_system_role}
                            />
                            {permission.action}
                        </label>
                    {/each}
                </div>
            {/each}
        </div>
    </div>
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex gap-2 justify-end">
        <Button size="sm" variant="outline" onclick={() => dialog?.close()}>
            {editing?.is_system_role ? "Close" : "Cancel"}
        </Button>
        {#if !editing?.is_system_role}
            <Button size="sm" onclick={save} disabled={saving || name.trim() === ""}>
                {saving ? "Saving..." : "Save"}
            </Button>
        {/if}
    </div>
</ModalDialog>
