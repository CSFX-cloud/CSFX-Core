<script lang="ts">
    import { Check, Copy, Eye, EyeOff, Pencil, Plus, Trash2, Undo2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteSecret, restoreSecret, type RevealedSecret, type SecretSummary } from "$lib/api/vaults";
    import { createClipboard } from "$lib/utils/clipboard.svelte";
    import RevealDialog from "./reveal-dialog.svelte";
    import SecretDialog from "./secret-dialog.svelte";

    const REVEAL_VISIBLE_MS = 30_000;
    const MASK = "•".repeat(12);

    type Props = {
        rgId: string;
        vaultId: string;
        secrets: SecretSummary[];
        onChanged: () => Promise<void>;
        onError: (message: string) => void;
    };

    let { rgId, vaultId, secrets, onChanged, onError }: Props = $props();

    const clipboard = createClipboard();

    let secretDialog = $state<SecretDialog | null>(null);
    let revealDialog = $state<RevealDialog | null>(null);
    let revealed = $state<Record<string, string>>({});

    function hide(name: string) {
        const { [name]: _removed, ...rest } = revealed;
        revealed = rest;
    }

    function show(secret: RevealedSecret) {
        revealed = { ...revealed, [secret.name]: secret.value };
        setTimeout(() => hide(secret.name), REVEAL_VISIBLE_MS);
    }

    async function run(action: (token: string) => Promise<unknown>, failure: string) {
        if (!auth.token) return;
        try {
            await action(auth.token);
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : failure);
        }
    }

    function remove(secret: SecretSummary) {
        if (!confirm(`Delete ${secret.name}? It can be restored for the soft delete period.`)) return;
        run((token) => deleteSecret(token, rgId, vaultId, secret.name), "Failed to delete secret");
    }
</script>

<SecretDialog bind:this={secretDialog} {rgId} {vaultId} onSaved={onChanged} />
<RevealDialog bind:this={revealDialog} {rgId} {vaultId} onRevealed={show} />

<section class="cx-panel" aria-labelledby="kv-secrets">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="kv-secrets">Secrets</h2>
            <p class="cx-panel__sub">Values stay hidden until you reveal them. Every reveal is logged.</p>
        </div>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => secretDialog?.open(null)}>
            <Plus class="cx-i" aria-hidden="true" />
            New secret
        </button>
    </div>
    <div class="db-scroll">
        <table class="cx-table">
            <thead><tr><th>Name</th><th>Value</th><th>Version</th><th>Used by</th><th>Updated</th><th>Expires</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
            <tbody>
                {#each secrets as secret (secret.name)}
                    {@const value = revealed[secret.name]}
                    <tr>
                        <td>
                            <span class="kv-name">{secret.name}</span>
                            {#if secret.deleted}<span class="cx-badge cx-badge--warning">Deleted</span>{/if}
                        </td>
                        <td>
                            <span class="kv-val">
                                <code class:is-masked={value === undefined}>{value ?? MASK}</code>
                                {#if value !== undefined}
                                    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Hide value" title="Hide" onclick={() => hide(secret.name)}>
                                        <EyeOff class="cx-i" aria-hidden="true" />
                                    </button>
                                    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Copy value" title="Copy" onclick={() => clipboard.copy(value, secret.name)}>
                                        {#if clipboard.copiedKey === secret.name}<Check class="cx-i" aria-hidden="true" />{:else}<Copy class="cx-i" aria-hidden="true" />{/if}
                                    </button>
                                {:else if !secret.deleted}
                                    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Reveal value" title="Reveal" onclick={() => revealDialog?.open(secret.name)}>
                                        <Eye class="cx-i" aria-hidden="true" />
                                    </button>
                                {/if}
                            </span>
                        </td>
                        <td class="cx-num">v{secret.version}</td>
                        <td>
                            <div class="rs-chips">
                                {#each secret.used_by as consumer (consumer)}<span class="rs-chip">{consumer}</span>{/each}
                            </div>
                        </td>
                        <td class="cx-soft">{secret.updated_at.slice(0, 10)}</td>
                        <td class="cx-soft">{secret.expires_at ?? "—"}</td>
                        <td>
                            <div class="db-actions" style="justify-content: flex-end">
                                {#if secret.deleted}
                                    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Restore {secret.name}" title="Restore" onclick={() => run((token) => restoreSecret(token, rgId, vaultId, secret.name, secret.version), "Failed to restore secret")}>
                                        <Undo2 class="cx-i" aria-hidden="true" />
                                    </button>
                                {:else}
                                    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="New version of {secret.name}" title="New version" onclick={() => secretDialog?.open(secret)}>
                                        <Pencil class="cx-i" aria-hidden="true" />
                                    </button>
                                    <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Delete {secret.name}" title="Delete" onclick={() => remove(secret)}>
                                        <Trash2 class="cx-i" aria-hidden="true" />
                                    </button>
                                {/if}
                            </div>
                        </td>
                    </tr>
                {:else}
                    <tr><td colspan="7" class="cx-soft">No secrets yet.</td></tr>
                {/each}
            </tbody>
        </table>
    </div>
</section>
