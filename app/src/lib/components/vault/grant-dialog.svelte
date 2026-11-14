<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createGrant, type Capability } from "$lib/api/vaults";
    import { listUsers } from "$lib/api/users";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";

    const ALL_SECRETS = "*";
    const CAPABILITIES: { value: Capability; label: string; subject: "resource" | "user" }[] = [
        { value: "read_secrets", label: "Read secrets", subject: "resource" },
        { value: "reveal_secrets", label: "Reveal secrets", subject: "user" },
        { value: "manage", label: "Manage everything", subject: "user" },
    ];

    let { rgId, vaultId, secretNames, onGranted }: { rgId: string; vaultId: string; secretNames: string[]; onGranted: () => Promise<void> } = $props();

    const rg = useResourceGroupState();

    let dialog = $state<ModalDialog | null>(null);
    let users = $state<string[]>([]);
    let principal = $state("");
    let capability = $state<Capability>("read_secrets");
    let scope = $state(ALL_SECRETS);
    let requiresApproval = $state(false);
    let busy = $state(false);
    let error = $state<string | null>(null);

    const subject = $derived(CAPABILITIES.find((option) => option.value === capability)?.subject ?? "resource");
    const resources = $derived([...new Set(rg.workloads.map((workload) => workload.name))].sort());
    const candidates = $derived(subject === "user" ? users : resources);
    const scoped = $derived(capability === "read_secrets");

    export async function open() {
        principal = "";
        capability = "read_secrets";
        scope = ALL_SECRETS;
        requiresApproval = false;
        dialog?.open();
        if (auth.token) users = (await listUsers().catch(() => [])).map((user) => user.username);
    }

    function changeCapability(value: Capability) {
        capability = value;
        principal = "";
        if (value !== "read_secrets") scope = ALL_SECRETS;
    }

    async function submit() {
        if (!principal) return;
        busy = true;
        error = null;
        try {
            await createGrant(rgId, vaultId, {
                principal,
                capability,
                scope,
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
            <label class="cx-label" for="grant-capability">Can</label>
            <select class="cx-input" id="grant-capability" value={capability} onchange={(event) => changeCapability(event.currentTarget.value as Capability)}>
                {#each CAPABILITIES as option (option.value)}
                    <option value={option.value}>{option.label}</option>
                {/each}
            </select>
        </div>
        <div class="cx-field">
            <label class="cx-label" for="grant-principal">{subject === "user" ? "Who (user)" : "Who (resource)"}</label>
            <select class="cx-input" id="grant-principal" bind:value={principal} disabled={candidates.length === 0}>
                <option value="">Select</option>
                {#each candidates as candidate (candidate)}
                    <option value={candidate}>{candidate}</option>
                {/each}
            </select>
            {#if candidates.length === 0}
                <p class="cx-hint">{subject === "user" ? "No users found." : "No resources in this group yet."}</p>
            {/if}
        </div>
        {#if scoped}
            <div class="cx-field">
                <label class="cx-label" for="grant-scope">On</label>
                <select class="cx-input" id="grant-scope" bind:value={scope}>
                    <option value={ALL_SECRETS}>All secrets</option>
                    {#each secretNames as name (name)}
                        <option value={name}>{name}</option>
                    {/each}
                </select>
            </div>
        {/if}
        {#if capability === "reveal_secrets"}
            <label class="cx-check">
                <input type="checkbox" bind:checked={requiresApproval} />
                <span>Every reveal needs approval</span>
            </label>
        {/if}
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={busy || !principal} aria-busy={busy}>
                {busy ? "Granting" : "Grant"}
            </button>
        </div>
    </form>
</ModalDialog>
