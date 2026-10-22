<script lang="ts" generics="T extends string">
    type Props = { tabs: { id: T; label: string }[]; active: T; label: string; onselect: (id: T) => void };

    let { tabs, active, label, onselect }: Props = $props();
</script>

<div class="rd-tabs">
    <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label={label}>
        {#each tabs as tab (tab.id)}
            <button class="cx-tab" role="tab" type="button" aria-selected={active === tab.id} onclick={() => onselect(tab.id)}>{tab.label}</button>
        {/each}
    </div>
</div>
