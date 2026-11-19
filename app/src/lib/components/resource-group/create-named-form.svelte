<script lang="ts" generics="T">
    type Props = {
        hint: string;
        placeholder: string;
        inputId: string;
        failure: string;
        create: (name: string) => Promise<T>;
        onCreated: (created: T) => Promise<void>;
        oncancel: () => void;
    };

    let { hint, placeholder, inputId, failure, create, onCreated, oncancel }: Props = $props();

    let name = $state("");
    let creating = $state(false);
    let error = $state<string | null>(null);

    async function submit() {
        if (!name.trim()) return;
        creating = true;
        error = null;
        try {
            const created = await create(name.trim());
            await onCreated(created);
        } catch (e) {
            error = e instanceof Error ? e.message : failure;
        } finally {
            creating = false;
        }
    }
</script>

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
        <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={oncancel}>Cancel</button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !name.trim()} aria-busy={creating}>
            {creating ? "Creating" : "Create"}
        </button>
    </div>
</form>
