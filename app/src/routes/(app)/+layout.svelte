<script lang="ts">
    import AppSidebar from "$lib/components/sidebar/app-sidebar.svelte";
    import SettingsDialog from "$lib/components/settings/settings-dialog.svelte";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { auth } from "$lib/auth/store.svelte";
    import { validateSession } from "$lib/auth/api";
    import { goto } from "$app/navigation";
    import { onMount } from "svelte";

    let { children } = $props();

    onMount(async () => {
        auth.init();
        if (!auth.token) {
            goto("/login");
            return;
        }
        try {
            const profile = await validateSession(auth.token);
            auth.setUser(profile);
        } catch {
            auth.clearSession();
            goto("/login");
        }
    });
</script>

<Sidebar.Provider>
    <AppSidebar />
    <Sidebar.Inset>
        {@render children()}
    </Sidebar.Inset>
</Sidebar.Provider>
<SettingsDialog />
