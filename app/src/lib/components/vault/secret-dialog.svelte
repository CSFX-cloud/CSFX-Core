<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createSecret, updateSecret, type SecretInput, type SecretSummary } from "$lib/api/vaults";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    let { rgId, vaultId, onSaved }: { rgId: string; vaultId: string; onSaved: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let existing = $state<SecretSummary | null>(null);
    let name = $state("");
    let value = $state("");
    let expiresAt = $state("");
    let saving = $state(false);
    let error = $state<string | null>(null);

    export function open(secret: SecretSummary | null) {
        existing = secret;
        name = secret?.name ?? "";
        value = "";
        expiresAt = secret?.expires_at ?? "";
        dialog?.open();
    }

    function payload(): SecretInput {
        return { value, expires_at: expiresAt || null };
    }

    async function submit() {
        if (!auth.token || !value) return;
        saving = true;
        error = null;
        try {
            if (existing) await updateSecret(auth.token, rgId, vaultId, existing.name, payload());
            else await createSecret(auth.token, rgId, vaultId, { ...payload(), name: name.trim() });
            dialog?.close();
            await onSaved();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to save secret";
        } finally {
            saving = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title={existing ? `New version of ${existing.name}` : "New secret"} onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        {#if !existing}
            <div class="cx-field">
                <label class="cx-label" for="secret-name">Name</label>
                <input class="cx-input" id="secret-name" placeholder="DATABASE_URL" autocomplete="off" bind:value={name} />
            </div>
        {/if}
        <div class="cx-field">
            <label class="cx-label" for="secret-value">Value</label>
            <input class="cx-input" id="secret-value" type="password" autocomplete="off" bind:value />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="secret-expires">Expires</label>
            <input class="cx-input" id="secret-expires" type="date" bind:value={expiresAt} />
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving || !value || (!existing && !name.trim())} aria-busy={saving}>
                {saving ? "Saving" : "Save"}
            </button>
        </div>
    </form>
</ModalDialog>
