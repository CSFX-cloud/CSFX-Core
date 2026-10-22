<script lang="ts">
    import "./layout.css";
    import { browser } from "$app/environment";
    import { Toaster } from "$lib/components/ui/sonner/index.js";

    let { children } = $props();

    if (browser) document.documentElement.classList.add("dark");
</script>

<svelte:head>
    <link rel="icon" type="image/svg+xml" href="/logo/logo-csfx.svg" />
</svelte:head>

{@render children()}
<Toaster theme="dark" />
