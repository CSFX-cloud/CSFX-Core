<script lang="ts">
    import type { Component, Snippet } from "svelte";
    import { page } from "$app/state";
    import UserIcon from "@lucide/svelte/icons/user";
    import SlidersHorizontalIcon from "@lucide/svelte/icons/sliders-horizontal";
    import DownloadIcon from "@lucide/svelte/icons/download";
    import ScrollTextIcon from "@lucide/svelte/icons/scroll-text";
    import KeyRoundIcon from "@lucide/svelte/icons/key-round";
    import ShieldCheckIcon from "@lucide/svelte/icons/shield-check";
    import RgTopbar from "$lib/components/resource-group/rg-topbar.svelte";

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
        {
            title: "Personal",
            links: [{ href: `${BASE}/account`, label: "Account", icon: UserIcon }],
        },
        {
            title: "System",
            links: [
                { href: `${BASE}/general`, label: "General", icon: SlidersHorizontalIcon },
                { href: `${BASE}/update`, label: "Update", icon: DownloadIcon },
                { href: `${BASE}/logs`, label: "Logs", icon: ScrollTextIcon },
            ],
        },
        {
            title: "Access",
            links: [
                { href: `${BASE}/identity-providers`, label: "Identity providers", icon: KeyRoundIcon },
                { href: `${BASE}/roles`, label: "Roles", icon: ShieldCheckIcon },
            ],
        },
    ];

    let { children }: { children: Snippet } = $props();
</script>

<RgTopbar backHref="/" backLabel="Back to dashboard" />

<div class="flex min-h-0 flex-1">
    <nav class="hidden md:flex flex-col w-64 shrink-0 gap-5 overflow-y-auto border-r border-border p-4">
        <h1 class="text-lg font-light text-foreground/70 px-1">Settings</h1>
        {#each groups as group (group.title)}
            <div class="flex flex-col gap-1">
                <h3 class="px-1 pb-1 text-xs font-semibold">{group.title}</h3>
                {#each group.links as link (link.href)}
                    <a
                        href={link.href}
                        class="flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm transition-colors {page.url.pathname.startsWith(link.href)
                            ? 'bg-primary/10 text-primary font-medium'
                            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
                    >
                        <link.icon class="size-4" />
                        {link.label}
                    </a>
                {/each}
            </div>
        {/each}
    </nav>
    <div class="flex min-w-0 flex-1 flex-col overflow-y-auto p-6">
        {@render children()}
    </div>
</div>
