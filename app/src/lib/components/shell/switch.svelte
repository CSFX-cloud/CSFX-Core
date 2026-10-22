<script lang="ts">
    type Props = { checked: boolean; label: string; disabled?: boolean; onchange: (checked: boolean) => void };

    let { checked, label, disabled = false, onchange }: Props = $props();
</script>

<button class="sw" type="button" role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={() => onchange(!checked)}></button>
