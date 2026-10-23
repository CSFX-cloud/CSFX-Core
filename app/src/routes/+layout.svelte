<script lang="ts">
    import "./layout.css";
    import { onMount } from "svelte";
    import { theme } from "$lib/utils/theme.svelte";
    import { Toaster } from "$lib/components/ui/sonner/index.js";

    let { children } = $props();

    onMount(theme.init);
</script>

<svelte:head>
    <link rel="icon" type="image/svg+xml" href="/logo/logo-csfx.svg" />
</svelte:head>

{@render children()}
<Toaster theme={theme.resolved} />
