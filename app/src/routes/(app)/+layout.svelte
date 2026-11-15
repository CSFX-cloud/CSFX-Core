<script lang="ts">
    import "@fontsource-variable/inter/index.css";
    import "$lib/styles/csfx-design-system.css";
    import "$lib/styles/csfx-app.css";
    import AppSidebar from "$lib/components/shell/app-sidebar.svelte";
    import AppTopbar from "$lib/components/shell/app-topbar.svelte";
    import CommandPalette from "$lib/components/command-palette/command-palette.svelte";

    let { children } = $props();
</script>

<div class="cx-root">
    <div class="cx-shell db-shell">
        <AppSidebar />
        <div class="db-main">
            <AppTopbar />
            <main class="db-view">
                {@render children()}
            </main>
        </div>
    </div>
</div>
<CommandPalette />
