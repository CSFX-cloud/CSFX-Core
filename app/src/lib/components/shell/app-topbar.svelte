<script lang="ts">
    import { page } from "$app/state";
    import { Bell } from "@lucide/svelte";
    import { findSection } from "$lib/components/shell/navigation";

    const section = $derived(findSection(page.url.pathname));
</script>

<header class="cx-topbar">
    <span class="cx-topbar__crumb">
        <section.icon class="cx-i" aria-hidden="true" />
        {section.title}
    </span>
    <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Notifications">
        <Bell class="cx-i" aria-hidden="true" />
    </button>
</header>
