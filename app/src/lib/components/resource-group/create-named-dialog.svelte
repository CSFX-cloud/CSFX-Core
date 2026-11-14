<script lang="ts" generics="T">
    import { auth } from "$lib/auth/store.svelte";
    import ModalDialog from "./modal-dialog.svelte";

    type Props = {
        title: string;
        subtitle: string;
        hint: string;
        placeholder: string;
        inputId: string;
        failure: string;
        create: (token: string, name: string) => Promise<T>;
        onCreated: (created: T) => Promise<void>;
    };

    let { title, subtitle, hint, placeholder, inputId, failure, create, onCreated }: Props = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let creating = $state(false);
    let error = $state<string | null>(null);

    export function open() {
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !name.trim()) return;
        creating = true;
        error = null;
        try {
            const created = await create(auth.token, name.trim());
            dialog?.close();
            name = "";
            await onCreated(created);
        } catch (e) {
            error = e instanceof Error ? e.message : failure;
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} {title} {subtitle} onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for={inputId}>Name</label>
            <input class="cx-input" id={inputId} {placeholder} autocomplete="off" bind:value={name} />
            <p class="cx-hint">{hint}</p>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !name.trim()} aria-busy={creating}>
                {creating ? "Creating" : "Create"}
            </button>
        </div>
    </form>
</ModalDialog>
