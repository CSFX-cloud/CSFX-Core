<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/state";
    import { auth } from "$lib/auth/store.svelte";
    import { notifications } from "$lib/components/shell/notifications.svelte";
    import { Bell } from "@lucide/svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import { NOTIFICATIONS_NAV, findSection } from "$lib/components/shell/navigation";

    const REFRESH_INTERVAL_MS = 60_000;

    const section = $derived(findSection(page.url.pathname));
    const label = $derived(notifications.openCount ? `Notifications, ${notifications.openCount} open` : "Notifications");

    onMount(() => {
        const refresh = () => auth.token && notifications.refresh(auth.token);
        refresh();
        const timer = setInterval(refresh, REFRESH_INTERVAL_MS);
        return () => clearInterval(timer);
    });
</script>

<header class="cx-topbar">
    <nav class="cx-topbar__crumb" aria-label="Breadcrumb">
        <section.icon class="cx-i" aria-hidden="true" />
        {#if breadcrumb.detail}
            <a class="db-crumb__link" href={section.href}>{section.title}</a>
            <span class="db-crumb__separator">/</span>
            <span class="db-crumb__current">{breadcrumb.detail}</span>
        {:else}
            {section.title}
        {/if}
    </nav>
    <a class="cx-icon-btn cx-icon-btn--sm nt-bell" href={NOTIFICATIONS_NAV.href} aria-label={label} aria-current={section === NOTIFICATIONS_NAV ? "page" : undefined}>
        <Bell class="cx-i" aria-hidden="true" fill={notifications.openCount ? "currentColor" : "none"} />
        {#if notifications.openCount}<span class="nt-count" aria-hidden="true">{notifications.openCount > 99 ? "99+" : notifications.openCount}</span>{/if}
    </a>
</header>
