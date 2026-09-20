<script lang="ts">
    import { Badge } from "$lib/components/ui/badge/index.js";
    import { isTransientStatus, statusLabel, statusVariant } from "$lib/utils/status.js";

    let {
        status,
        label: labelOverride,
        class: className = "",
        title,
    }: { status: string; label?: string; class?: string; title?: string } = $props();

    const variant = $derived(statusVariant(status));
    const label = $derived(labelOverride ?? statusLabel(status));
    const loading = $derived(isTransientStatus(status));
</script>

<Badge {variant} {loading} class={className} {title}>{label}</Badge>
