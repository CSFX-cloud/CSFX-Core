<script lang="ts">
    import "@fontsource-variable/inter/index.css";
    import "$lib/styles/csfx-design-system.css";
    import "$lib/styles/csfx-app.css";
    import AppSidebar from "$lib/components/shell/app-sidebar.svelte";
    import AppTopbar from "$lib/components/shell/app-topbar.svelte";
    import CommandPalette from "$lib/components/command-palette/command-palette.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { validateSession } from "$lib/auth/api";
    import { ApiError } from "$lib/api/http";
    import { goto } from "$app/navigation";
    import { onMount } from "svelte";

    let { children } = $props();

    auth.init();

    onMount(async () => {
        if (!auth.token) {
            goto("/login");
            return;
        }
        try {
            auth.setUser(await validateSession());
        } catch (e) {
            if (!(e instanceof ApiError && e.status === 401)) throw e;
        }
    });
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
