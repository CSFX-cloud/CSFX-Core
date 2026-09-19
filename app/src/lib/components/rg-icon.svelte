<script lang="ts">
    import Icon from "@iconify/svelte";
    import { resourceGroupIconImageUrl } from "$lib/api/resource-groups";

    let {
        id,
        icon,
        color,
        hasIconImage,
        size = 18,
    }: {
        id: string;
        icon: string;
        color: string;
        hasIconImage: boolean;
        size?: number;
    } = $props();
</script>

{#if hasIconImage}
    <img
        src={resourceGroupIconImageUrl(id)}
        alt=""
        class="rounded-lg object-cover"
        style="width: {size}px; height: {size}px;"
    />
{:else}
    <Icon icon={icon || "mdi:cube-outline"} width={size} height={size} style="color: {color};" />
{/if}
