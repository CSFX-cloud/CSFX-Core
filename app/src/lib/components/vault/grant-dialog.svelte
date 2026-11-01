<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createGrant, type Capability } from "$lib/api/vaults";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    const CAPABILITIES: { value: Capability; label: string }[] = [
        { value: "read_secrets", label: "Read secrets" },
        { value: "sign_with_key", label: "Sign with key" },
        { value: "reveal_secrets", label: "Reveal secrets" },
        { value: "manage", label: "Manage everything" },
    ];

    let { rgId, vaultId, onGranted }: { rgId: string; vaultId: string; onGranted: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let principal = $state("");
    let capability = $state<Capability>("read_secrets");
    let scope = $state("*");
    let requiresApproval = $state(false);
    let busy = $state(false);
    let error = $state<string | null>(null);

    export function open() {
        principal = "";
        capability = "read_secrets";
        scope = "*";
        requiresApproval = false;
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !principal.trim() || !scope.trim()) return;
        busy = true;
        error = null;
        try {
            await createGrant(auth.token, rgId, vaultId, {
                principal: principal.trim(),
                capability,
                scope: scope.trim(),
                requires_approval: capability === "reveal_secrets" && requiresApproval,
            });
            dialog?.close();
            await onGranted();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to grant access";
        } finally {
            busy = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Grant access" subtitle="Resources sign in with their own identity, no passwords needed." onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="grant-principal">Who</label>
            <input class="cx-input" id="grant-principal" placeholder="app-stack" autocomplete="off" bind:value={principal} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="grant-capability">Can</label>
            <select class="cx-input" id="grant-capability" bind:value={capability}>
                {#each CAPABILITIES as option (option.value)}
                    <option value={option.value}>{option.label}</option>
                {/each}
            </select>
        </div>
        <div class="cx-field">
            <label class="cx-label" for="grant-scope">On</label>
            <input class="cx-input" id="grant-scope" placeholder="DATABASE_URL or SMTP_*" autocomplete="off" bind:value={scope} />
        </div>
        {#if capability === "reveal_secrets"}
            <label class="cx-check">
                <input type="checkbox" bind:checked={requiresApproval} />
                <span>Every reveal needs approval</span>
            </label>
        {/if}
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={busy || !principal.trim() || !scope.trim()} aria-busy={busy}>
                {busy ? "Granting" : "Grant"}
            </button>
        </div>
    </form>
</ModalDialog>
