<script lang="ts">
    import Icon from "@iconify/svelte";
    import { serviceIcon } from "$lib/components/shell/service-icons";

    const BRAND_SIZES = { md: 18, lg: 22 };

    type Props = { name: string; size?: "md" | "lg"; brand?: string | null };

    let { name, size = "md", brand = null }: Props = $props();

    const icon = $derived(serviceIcon(name));
</script>

<span class="svc-tile {size === 'lg' ? 'svc-tile--lg' : ''}" style="background: {icon.tile}" aria-hidden="true">
    {#if brand}
        <Icon icon={brand} width={BRAND_SIZES[size]} height={BRAND_SIZES[size]} />
    {:else}
        <svg class="cx-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            {@html icon.svg}
        </svg>
    {/if}
</span>
