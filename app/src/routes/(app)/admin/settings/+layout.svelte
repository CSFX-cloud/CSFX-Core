<script lang="ts">
    import type { Component, Snippet } from "svelte";
    import { Bell, KeyRound, ScrollText, ShieldCheck, SlidersHorizontal, User, Users, Download } from "@lucide/svelte";
    import { page } from "$app/state";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";

    interface SettingsLink {
        href: string;
        label: string;
        icon: Component;
    }

    interface SettingsGroup {
        title: string;
        links: SettingsLink[];
    }

    const BASE = "/admin/settings";

    const groups: SettingsGroup[] = [
        { title: "Personal", links: [{ href: `${BASE}/account`, label: "Account", icon: User }] },
        {
            title: "System",
            links: [
                { href: `${BASE}/general`, label: "General", icon: SlidersHorizontal },
                { href: `${BASE}/update`, label: "Update", icon: Download },
                { href: `${BASE}/logs`, label: "Logs", icon: ScrollText },
                { href: `${BASE}/notifications`, label: "Notifications", icon: Bell },
            ],
        },
        {
            title: "Access",
            links: [
                { href: `${BASE}/users`, label: "Users", icon: Users },
                { href: `${BASE}/identity-providers`, label: "Identity providers", icon: KeyRound },
                { href: `${BASE}/roles`, label: "Roles", icon: ShieldCheck },
            ],
        },
    ];

    let { children }: { children: Snippet } = $props();

    const current = $derived(groups.flatMap((group) => group.links).find((link) => page.url.pathname.startsWith(link.href)));

    $effect(() => {
        breadcrumb.set(current?.label ?? null);
        return () => breadcrumb.set(null);
    });
</script>

<div class="st-main">
    <nav class="st-nav" aria-label="Settings">
        <h1>Settings</h1>
        {#each groups as group, index (group.title)}
            <div class="cx-side__section" style={index === 0 ? "padding-top: 4px" : ""}>{group.title}</div>
            {#each group.links as link (link.href)}
                <a class="cx-side__item" href={link.href} aria-current={page.url.pathname.startsWith(link.href) ? "page" : undefined}>
                    <link.icon class="cx-i" aria-hidden="true" />{link.label}
                </a>
            {/each}
        {/each}
    </nav>
    <div class="st-body">
        {@render children()}
    </div>
</div>
