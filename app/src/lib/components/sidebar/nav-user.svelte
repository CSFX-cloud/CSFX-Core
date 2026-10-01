<script lang="ts">
    import * as Avatar from "$lib/components/ui/avatar/index.js";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { useSidebar } from "$lib/components/ui/sidebar/index.js";
    import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
    import LogOutIcon from "@lucide/svelte/icons/log-out";
    import UserIcon from "@lucide/svelte/icons/user";
    import { auth } from "$lib/auth/store.svelte";
    import { gravatarUrl, getAvatarFallbackStyle, type AvatarFallbackStyle } from "$lib/auth/api";
    import { Blobatar } from "@blobatar/svelte";
    import { gaze } from "@blobatar/svelte/gaze";
    import "blobatar/motion.css";
    import "blobatar/gaze.css";
    import { settingsDialog } from "$lib/components/settings/settings-store.svelte.js";
    import { goto } from "$app/navigation";

    const sidebar = useSidebar();

    let avatarUrl = $state<string | null>(null);
    let fallbackStyle = $state<AvatarFallbackStyle>("initials");

    $effect(() => {
        const source = auth.user?.gravatar_email;
        if (!source) {
            avatarUrl = null;
            return;
        }
        gravatarUrl(source, 64).then((url) => (avatarUrl = url));
    });

    $effect(() => {
        if (!auth.token) return;
        getAvatarFallbackStyle(auth.token)
            .then((style) => (fallbackStyle = style))
            .catch(() => {});
    });

    function initials(name: string): string {
        return name.slice(0, 2).toUpperCase();
    }

    const triggerEyes = gaze({ travel: 3, target: "pointer" });
    const menuEyes = gaze({ travel: 3, target: "pointer" });

    function logout() {
        auth.clearSession();
        goto("/login");
    }
</script>

<Sidebar.Menu>
    <Sidebar.MenuItem>
        <DropdownMenu.Root>
            <DropdownMenu.Trigger>
                {#snippet child({ props })}
                    <Sidebar.MenuButton
                        size="lg"
                        class="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
                        {...props}
                    >
                        <Avatar.Root class="size-8 rounded-lg {fallbackStyle === 'blobatar' ? 'after:border-0' : ''}">
                            {#if avatarUrl}
                                <Avatar.Image src={avatarUrl} alt={auth.user?.username ?? "avatar"} />
                            {/if}
                            <Avatar.Fallback class="rounded-lg text-xs {fallbackStyle === 'blobatar' ? 'bg-transparent p-0' : ''}">
                                {#if fallbackStyle === "blobatar" && auth.user}
                                    <Blobatar {@attach triggerEyes} name={auth.user.username} size={32} class="size-full scale-[1.8]" animate="always" />
                                {:else}
                                    {auth.user ? initials(auth.user.username) : "??"}
                                {/if}
                            </Avatar.Fallback>
                        </Avatar.Root>
                        <div class="grid flex-1 text-start text-sm leading-tight">
                            <span class="truncate font-medium">{auth.user?.username ?? "-"}</span>
                            <span class="truncate text-xs text-muted-foreground">{auth.user?.email ?? ""}</span>
                        </div>
                        <ChevronsUpDownIcon class="ms-auto size-4" />
                    </Sidebar.MenuButton>
                {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content
                class="w-(--bits-dropdown-menu-anchor-width) min-w-56 rounded-lg"
                side={sidebar.isMobile ? "bottom" : "right"}
                align="end"
                sideOffset={4}
            >
                <DropdownMenu.Label class="p-0 font-normal">
                    <div class="flex items-center gap-2 px-1 py-1.5 text-start text-sm">
                        <Avatar.Root class="size-8 rounded-lg {fallbackStyle === 'blobatar' ? 'after:border-0' : ''}">
                            {#if avatarUrl}
                                <Avatar.Image src={avatarUrl} alt={auth.user?.username ?? "avatar"} />
                            {/if}
                            <Avatar.Fallback class="rounded-lg text-xs {fallbackStyle === 'blobatar' ? 'bg-transparent p-0' : ''}">
                                {#if fallbackStyle === "blobatar" && auth.user}
                                    <Blobatar {@attach menuEyes} name={auth.user.username} size={32} class="size-full" animate="always" />
                                {:else}
                                    {auth.user ? initials(auth.user.username) : "??"}
                                {/if}
                            </Avatar.Fallback>
                        </Avatar.Root>
                        <div class="grid flex-1 text-start text-sm leading-tight">
                            <span class="truncate font-medium">{auth.user?.username ?? "-"}</span>
                            <span class="truncate text-xs text-muted-foreground">{auth.user?.email ?? ""}</span>
                        </div>
                    </div>
                </DropdownMenu.Label>
                <DropdownMenu.Separator />
                <DropdownMenu.Group>
                    <DropdownMenu.Item onclick={() => settingsDialog.show()}>
                        <UserIcon />
                        Profile
                    </DropdownMenu.Item>
                </DropdownMenu.Group>
                <DropdownMenu.Separator />
                <DropdownMenu.Item onclick={logout}>
                    <LogOutIcon />
                    Log out
                </DropdownMenu.Item>
            </DropdownMenu.Content>
        </DropdownMenu.Root>
    </Sidebar.MenuItem>
</Sidebar.Menu>
