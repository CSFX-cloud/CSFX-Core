<script lang="ts">
    import { Eye, EyeOff } from "@lucide/svelte";

    type Props = {
        id: string;
        value?: string;
        autocomplete: "new-password" | "current-password" | "off";
        placeholder?: string;
        invalid?: boolean;
    };

    let { id, value = $bindable(""), autocomplete, placeholder, invalid = false }: Props = $props();

    let visible = $state(false);
</script>

<div class="cx-input-wrap">
    <input
        class="cx-input cx-input--action"
        {id}
        type={visible ? "text" : "password"}
        {autocomplete}
        {placeholder}
        aria-invalid={invalid}
        bind:value
    />
    <button
        class="cx-icon-btn cx-icon-btn--sm"
        type="button"
        aria-label={visible ? "Hide password" : "Show password"}
        aria-pressed={visible}
        onclick={() => (visible = !visible)}
    >
        {#if visible}<EyeOff class="cx-i" aria-hidden="true" />{:else}<Eye class="cx-i" aria-hidden="true" />{/if}
    </button>
</div>
