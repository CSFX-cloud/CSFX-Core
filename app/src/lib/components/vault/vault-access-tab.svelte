<script lang="ts">
    import { Plus, Trash2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteGrant, type Capability, type VaultAuditEntry, type VaultGrant } from "$lib/api/vaults";
    import GrantDialog from "./grant-dialog.svelte";

    const CAPABILITY_LABELS: Record<Capability, string> = {
        read_secrets: "read secrets",
        sign_with_key: "sign with key",
        reveal_secrets: "reveal secrets",
        manage: "manage everything",
    };
    const RECOVERY_QUORUM = "3 of 5 people";

    type Props = {
        rgId: string;
        vaultId: string;
        grants: VaultGrant[];
        audit: VaultAuditEntry[];
        softDeleteDays: number;
        onChanged: () => Promise<void>;
        onError: (message: string) => void;
    };

    let { rgId, vaultId, grants, audit, softDeleteDays, onChanged, onError }: Props = $props();

    let grantDialog = $state<GrantDialog | null>(null);

    async function revoke(grant: VaultGrant) {
        if (!auth.token || !confirm(`Revoke ${CAPABILITY_LABELS[grant.capability]} for ${grant.principal}?`)) return;
        try {
            await deleteGrant(auth.token, rgId, vaultId, grant.id);
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to revoke access");
        }
    }

    const denied = (entry: VaultAuditEntry) => entry.result.startsWith("denied");
</script>

<GrantDialog bind:this={grantDialog} {rgId} {vaultId} onGranted={onChanged} />

<div class="rs-stack">
    <div class="rs-cols">
        <section class="cx-panel" aria-labelledby="kv-access">
            <div class="cx-panel__head">
                <div>
                    <h2 class="cx-panel__title" id="kv-access">Who can use what</h2>
                    <p class="cx-panel__sub">Resources sign in with their own identity, no passwords needed</p>
                </div>
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => grantDialog?.open()}>
                    <Plus class="cx-i" aria-hidden="true" />
                    Grant access
                </button>
            </div>
            <div class="db-scroll">
                <table class="cx-table">
                    <thead><tr><th>Who</th><th>Can</th><th>On</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
                    <tbody>
                        {#each grants as grant (grant.id)}
                            <tr>
                                <td>{grant.principal}</td>
                                <td class="cx-soft">{CAPABILITY_LABELS[grant.capability]}</td>
                                <td class="cx-num">{grant.requires_approval ? "needs approval" : grant.scope}</td>
                                <td>
                                    <div class="db-actions" style="justify-content: flex-end">
                                        <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Revoke access of {grant.principal}" title="Revoke" onclick={() => revoke(grant)}>
                                            <Trash2 class="cx-i" aria-hidden="true" />
                                        </button>
                                    </div>
                                </td>
                            </tr>
                        {:else}
                            <tr><td colspan="4" class="cx-soft">No access granted yet.</td></tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </section>
        <section class="cx-panel" aria-labelledby="kv-protection" style="display: grid; gap: 14px">
            <div class="cx-panel__head" style="margin-bottom: 0">
                <h2 class="cx-panel__title" id="kv-protection">Protection</h2>
            </div>
            <dl class="rd-kv">
                <div><dt>Soft delete</dt><dd>{softDeleteDays} days</dd></div>
                <div><dt>Reveal needs</dt><dd>2FA + reason</dd></div>
                <div><dt>Recovery keys</dt><dd>{RECOVERY_QUORUM}</dd></div>
            </dl>
        </section>
    </div>

    <section class="cx-panel" aria-labelledby="kv-audit">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="kv-audit">Audit log</h2>
                <p class="cx-panel__sub">Every read, reveal and change. Can't be edited or deleted.</p>
            </div>
        </div>
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Time</th><th>Who</th><th>Action</th><th>From</th><th>Result</th></tr></thead>
                <tbody>
                    {#each audit as entry (entry.id)}
                        <tr>
                            <td class="cx-num" title={entry.occurred_at}>{entry.occurred_at.slice(0, 19).replace("T", " ")}</td>
                            <td>{entry.actor}</td>
                            <td class="cx-soft">{entry.action}</td>
                            <td class="cx-num">{entry.source}</td>
                            <td class="cx-num {denied(entry) ? 'kv-res-no' : 'kv-res-ok'}">{entry.result}</td>
                        </tr>
                    {:else}
                        <tr><td colspan="5" class="cx-soft">Nothing recorded yet.</td></tr>
                    {/each}
                </tbody>
            </table>
        </div>
    </section>
</div>
