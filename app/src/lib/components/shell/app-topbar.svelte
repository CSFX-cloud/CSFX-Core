<script lang="ts">
    import { page } from "$app/state";
    import { Bell } from "@lucide/svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import { findSection } from "$lib/components/shell/navigation";

    const section = $derived(findSection(page.url.pathname));
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
    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Notifications">
        <Bell class="cx-i" aria-hidden="true" />
    </button>
</header>
