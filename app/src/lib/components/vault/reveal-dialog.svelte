<script lang="ts">
    import { revealSecret, type RevealedSecret } from "$lib/api/vaults";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    const CODE_LENGTH = 6;

    let { rgId, vaultId, onRevealed }: { rgId: string; vaultId: string; onRevealed: (secret: RevealedSecret) => void } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let secretName = $state("");
    let reason = $state("");
    let code = $state("");
    let busy = $state(false);
    let error = $state<string | null>(null);

    const ready = $derived(reason.trim() !== "" && code.length === CODE_LENGTH && !busy);

    export function open(name: string) {
        secretName = name;
        reason = "";
        code = "";
        dialog?.open();
    }

    async function submit() {
        if (!ready) return;
        busy = true;
        error = null;
        try {
            onRevealed(await revealSecret(rgId, vaultId, secretName, { reason: reason.trim(), code }));
            dialog?.close();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to reveal secret";
        } finally {
            busy = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Reveal {secretName}" subtitle="Needs two factor authentication and a reason. Every reveal is logged." onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="reveal-reason">Reason</label>
            <input class="cx-input" id="reveal-reason" maxlength="500" autocomplete="off" bind:value={reason} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="reveal-code">Authenticator code</label>
            <input class="cx-input" id="reveal-code" inputmode="numeric" maxlength={CODE_LENGTH} autocomplete="one-time-code" bind:value={code} />
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={!ready} aria-busy={busy}>
                {busy ? "Checking" : "Reveal"}
            </button>
        </div>
    </form>
</ModalDialog>
