<script lang="ts">
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import { ArrowUpCircle, ChevronsUpDown, LogOut, Search } from "@lucide/svelte";
    import { getReleases, getUpdateStatus } from "$lib/api/system";
    import { auth } from "$lib/auth/store.svelte";
    import { commandPalette } from "$lib/components/command-palette/command-palette-store.svelte.js";
    import UserAvatar from "$lib/components/shell/user-avatar.svelte";
    import { isActive, PRIMARY_NAV, RESOURCE_NAV, SETTINGS_NAV, type NavItem } from "$lib/components/shell/navigation";

    let version = $state<string | null>(null);
    let latestStable = $state<string | null>(null);
    let menuOpen = $state(false);
    let account = $state<HTMLElement>();

    async function loadVersions(token: string) {
        const [status, releases] = await Promise.allSettled([getUpdateStatus(token), getReleases(token)]);
        if (status.status === "fulfilled") version = status.value.current_version;
        if (releases.status === "fulfilled" && releases.value.update_available) {
            latestStable = releases.value.latest_stable;
        }
    }

    $effect(() => {
        const token = auth.token;
        if (token) loadVersions(token);
    });

    function logout() {
        menuOpen = false;
        auth.clearSession();
        goto("/login");
    }

    function closeOnOutsideClick(event: MouseEvent) {
        if (menuOpen && !account?.contains(event.target as Node)) menuOpen = false;
    }

    function closeOnEscape(event: KeyboardEvent) {
        if (event.key === "Escape") menuOpen = false;
    }
</script>

{#snippet link(item: NavItem)}
    <a class="cx-side__item" href={item.href} aria-current={isActive(item.href, page.url.pathname) ? "page" : undefined}>
        <item.icon class="cx-i" aria-hidden="true" />{item.title}
    </a>
{/snippet}

<svelte:window onclick={closeOnOutsideClick} onkeydown={closeOnEscape} />

<aside class="cx-side db-side" aria-label="Main navigation">
    <div class="cx-side__brand db-brand">
        <span class="db-brand__name">
            <img class="db-brand__logo" src="/logo/logo-csfx.svg" alt="" />
            csfx.cloud
        </span>
        {#if version}<span class="db-version">v{version}</span>{/if}
    </div>
    <button class="cx-search db-search" type="button" onclick={() => commandPalette.show()}>
        <Search class="cx-i" aria-hidden="true" />
        Search
        <span class="db-kbd">&#8984;K</span>
    </button>

    {#each PRIMARY_NAV as item (item.href)}{@render link(item)}{/each}

    <div class="cx-side__section">Resources</div>
    {#each RESOURCE_NAV as item (item.href)}{@render link(item)}{/each}

    {#if latestStable}
        <a class="db-update" href="/admin/settings">
            <ArrowUpCircle class="cx-i" aria-hidden="true" />
            <span>Update available<small>{latestStable}</small></span>
        </a>
    {/if}

    <div class="db-side__foot">
        <div class="db-account" bind:this={account}>
            {#if menuOpen}
                <div class="db-menu" role="menu" aria-label="Account menu">
                    <a
                        class="cx-side__item"
                        role="menuitem"
                        href={SETTINGS_NAV.href}
                        onclick={() => (menuOpen = false)}
                    >
                        <SETTINGS_NAV.icon class="cx-i" aria-hidden="true" />{SETTINGS_NAV.title}
                    </a>
                    <div class="db-menu__separator" role="separator"></div>
                    <button class="cx-side__item db-action" type="button" role="menuitem" onclick={logout}>
                        <LogOut class="cx-i" aria-hidden="true" />Log out
                    </button>
                </div>
            {/if}
            <button
                class="cx-side__item db-action"
                type="button"
                aria-haspopup="menu"
                aria-expanded={menuOpen}
                onclick={() => (menuOpen = !menuOpen)}
            >
                <UserAvatar />
                <span style="flex: 1; overflow: hidden; text-overflow: ellipsis">{auth.user?.email ?? auth.user?.username ?? ""}</span>
                <ChevronsUpDown class="cx-i" aria-hidden="true" />
            </button>
        </div>
    </div>
</aside>
