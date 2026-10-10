<script lang="ts">
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";

    onMount(() => {
        goto("/admin/settings/account", { replaceState: true });
    });
</script>
