<script lang="ts">
    import type { ResourceGroup } from "$lib/api/resource-groups";
    import RgIcon from "$lib/components/rg-icon.svelte";

    const ICON_SIZES = { md: 18, lg: 22 };

    let { group, size = "md" }: { group: ResourceGroup; size?: "md" | "lg" } = $props();
</script>

<span class="tile tint {size === 'lg' ? 'tile--lg' : ''}" style="--tint: {group.color}" aria-hidden="true">
    <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={ICON_SIZES[size]} />
</span>
