<script lang="ts">
    import { createJoinToken, type JoinToken } from '$lib/api/nodes';
    import ModalDialog from '$lib/components/resource-group/modal-dialog.svelte';

    const TTL_OPTIONS = [
        { hours: 1, label: '1 hour' },
        { hours: 24, label: '24 hours' },
        { hours: 72, label: '3 days' },
    ];

    let dialog = $state<ModalDialog | null>(null);
    let role = $state<JoinToken['role']>('worker');
    let ttlHours = $state(1);
    let busy = $state(false);
    let error = $state<string | null>(null);
    let result = $state<JoinToken | null>(null);
    let copied = $state(false);

    export function open() {
        result = null;
        error = null;
        copied = false;
        dialog?.open();
    }

    async function generate() {
        busy = true;
        error = null;
        try {
            result = await createJoinToken(role, ttlHours);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to create join token';
        } finally {
            busy = false;
        }
    }

    async function copy() {
        if (!result) return;
        await navigator.clipboard.writeText(result.join_string);
        copied = true;
    }
</script>

<ModalDialog bind:this={dialog} title="Add node" size="md">
    {#if result}
        <p class="cx-hint">
            Set this value as CSFX_JOIN on the new {result.role} node, or enter it in the node console. It works once and expires at {new Date(result.expires_at).toLocaleString()}.
        </p>
        <code class="cx-mono" style="word-break: break-all">{result.join_string}</code>
        <div class="md__foot">
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => dialog?.close()}>Done</button>
        </div>
    {:else}
        <div class="md__fields">
            <div class="cx-field">
                <label class="cx-label" for="join-role">Role</label>
                <select id="join-role" class="cx-input" bind:value={role}>
                    <option value="worker">Worker</option>
                    <option value="master">Master (control plane)</option>
                </select>
            </div>
            <div class="cx-field">
                <label class="cx-label" for="join-ttl">Valid for</label>
                <select id="join-ttl" class="cx-input" bind:value={ttlHours}>
                    {#each TTL_OPTIONS as option (option.hours)}
                        <option value={option.hours}>{option.label}</option>
                    {/each}
                </select>
            </div>
            {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        </div>
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={generate} disabled={busy} aria-busy={busy}>
                {busy ? 'Creating' : 'Create join string'}
            </button>
        </div>
    {/if}
</ModalDialog>
