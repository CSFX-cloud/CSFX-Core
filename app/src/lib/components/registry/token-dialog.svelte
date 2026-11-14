<script lang="ts">
    import { Check, Copy } from "@lucide/svelte";
    import { createToken, type CreatedToken, type TokenScope } from "$lib/api/registries";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";
    import { createClipboard } from "$lib/utils/clipboard.svelte";

    type Props = { rgId: string; registryId: string; onCreated: () => Promise<void> };

    let { rgId, registryId, onCreated }: Props = $props();

    const clipboard = createClipboard();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let scope = $state<TokenScope>("push");
    let creating = $state(false);
    let error = $state<string | null>(null);
    let created = $state<CreatedToken | null>(null);

    export function open() {
        dialog?.open();
    }

    function reset() {
        name = "";
        scope = "push";
        error = null;
        created = null;
    }

    async function submit() {
        if (!name.trim()) return;
        creating = true;
        error = null;
        try {
            created = await createToken(rgId, registryId, name.trim(), scope);
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create access token";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="New access token" subtitle="Use it to log in with docker, buildah or skopeo." onclose={reset}>
    {#if created}
        <div class="md__fields">
            <p class="cx-hint">Copy the secret now. It is not shown again.</p>
            {#each [{ label: "Username", value: created.username }, { label: "Secret", value: created.secret }] as field (field.label)}
                <div class="cx-field">
                    <span class="cx-label">{field.label}</span>
                    <div style="display: flex; gap: 8px; align-items: center">
                        <code class="kv-name" style="overflow-wrap: anywhere">{field.value}</code>
                        <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Copy {field.label.toLowerCase()}" title="Copy" onclick={() => clipboard.copy(field.value, field.label)}>
                            {#if clipboard.copiedKey === field.label}<Check class="cx-i" aria-hidden="true" />{:else}<Copy class="cx-i" aria-hidden="true" />{/if}
                        </button>
                    </div>
                </div>
            {/each}
            <div class="md__foot">
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => dialog?.close()}>Done</button>
            </div>
        </div>
    {:else}
        <form
            class="md__fields"
            onsubmit={(event) => {
                event.preventDefault();
                submit();
            }}
        >
            <div class="cx-field">
                <label class="cx-label" for="rt-name">Name</label>
                <input class="cx-input" id="rt-name" placeholder="ci-pipeline" autocomplete="off" bind:value={name} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="rt-scope">Access</label>
                <select class="cx-input" id="rt-scope" bind:value={scope}>
                    <option value="push">Push and pull</option>
                    <option value="pull">Pull only</option>
                </select>
            </div>
            {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
            <div class="md__foot">
                <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !name.trim()} aria-busy={creating}>
                    {creating ? "Creating" : "Create token"}
                </button>
            </div>
        </form>
    {/if}
</ModalDialog>
