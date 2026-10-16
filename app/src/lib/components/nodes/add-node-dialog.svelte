<script lang="ts">
    import { auth } from '$lib/auth/store.svelte';
    import { createJoinToken, type JoinToken } from '$lib/api/nodes';
    import { Button } from '$lib/components/ui/button/index.js';

    const TTL_OPTIONS = [
        { hours: 1, label: '1 hour' },
        { hours: 24, label: '24 hours' },
        { hours: 72, label: '3 days' },
    ];

    let dialog = $state<HTMLDialogElement | null>(null);
    let ttlHours = $state(1);
    let busy = $state(false);
    let error = $state<string | null>(null);
    let result = $state<JoinToken | null>(null);
    let copied = $state(false);

    export function open() {
        result = null;
        error = null;
        copied = false;
        dialog?.showModal();
    }

    async function generate() {
        if (!auth.token) return;
        busy = true;
        error = null;
        try {
            result = await createJoinToken(auth.token, ttlHours);
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

<dialog
    bind:this={dialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-lg rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
>
    <div class="flex flex-col gap-4 p-6">
        <h2 class="text-base font-semibold">Add node</h2>
        {#if result}
            <p class="text-sm text-muted-foreground">
                Set this value as CSFX_JOIN on the new {result.role} node, or enter it in the node console. It works once and expires at {new Date(result.expires_at).toLocaleString()}.
            </p>
            <code class="block break-all rounded border bg-muted/40 px-3 py-2 text-xs">{result.join_string}</code>
            <div class="flex gap-2 justify-end">
                <Button size="sm" variant="outline" onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
                <Button size="sm" onclick={() => dialog?.close()}>Done</Button>
            </div>
        {:else}
            <p class="text-sm text-muted-foreground">Creates a single-use join string for a new worker node.</p>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="join-ttl">Valid for</label>
                <select id="join-ttl" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={ttlHours}>
                    {#each TTL_OPTIONS as option}
                        <option value={option.hours}>{option.label}</option>
                    {/each}
                </select>
            </div>
            {#if error}
                <p class="text-xs text-destructive">{error}</p>
            {/if}
            <div class="flex gap-2 justify-end">
                <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
                <Button size="sm" onclick={generate} disabled={busy}>
                    {busy ? 'Creating...' : 'Create join string'}
                </Button>
            </div>
        {/if}
    </div>
</dialog>
