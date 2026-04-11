<script lang="ts">
  import * as Avatar from '$lib/components/ui/avatar/index.js';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
  import * as Sidebar from '$lib/components/ui/sidebar/index.js';
  import { useSidebar } from '$lib/components/ui/sidebar/index.js';
  import BadgeCheckIcon from '@lucide/svelte/icons/badge-check';
  import BellIcon from '@lucide/svelte/icons/bell';
  import ChevronsUpDownIcon from '@lucide/svelte/icons/chevrons-up-down';
  import CreditCardIcon from '@lucide/svelte/icons/credit-card';
  import LogOutIcon from '@lucide/svelte/icons/log-out';
  import SparklesIcon from '@lucide/svelte/icons/sparkles';
  import UserIcon from '@lucide/svelte/icons/user';
  import { authStore } from '$lib/stores/auth';
  import { AuthService } from '$lib/services/auth';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  const sidebar = useSidebar();

  // Subscribe to auth store - use $derived for reactivity
  let authState = $derived($authStore);

  // Get user initials for avatar fallback
  let userInitials = $derived(
    authState.user?.username ? authState.user.username.substring(0, 2).toUpperCase() : 'U'
  );

  async function handleLogout() {
    try {
      if (authState.token) {
        await AuthService.logout(authState.token);
      }

      // Clear auth cookie
      await fetch('/api/set-auth-cookie', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token: null }),
      });

      // Clear store
      authStore.logout();

      // Redirect to signin
      await goto('/signin');
    } catch (error) {
      console.error('Logout failed:', error);
      // Even if API call fails, clear local state and redirect
      authStore.logout();
      await goto('/signin');
    }
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
            <Avatar.Root class="size-8 rounded-lg">
              <Avatar.Fallback
                class="rounded-lg bg-sidebar-primary text-sidebar-primary-foreground"
              >
                {userInitials}
              </Avatar.Fallback>
            </Avatar.Root>
            <div class="grid flex-1 text-left text-sm leading-tight">
              <span class="truncate font-medium">{authState.user?.username || 'User'}</span>
              <span class="truncate text-xs">CSFX-Core</span>
            </div>
            <ChevronsUpDownIcon class="ml-auto size-4" />
          </Sidebar.MenuButton>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        class="w-(--bits-dropdown-menu-anchor-width) min-w-56 rounded-lg"
        side={sidebar.isMobile ? 'bottom' : 'right'}
        align="end"
        sideOffset={4}
      >
        <DropdownMenu.Label class="p-0 font-normal">
          <div class="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
            <Avatar.Root class="size-8 rounded-lg">
              <Avatar.Fallback
                class="rounded-lg bg-sidebar-primary text-sidebar-primary-foreground"
              >
                {userInitials}
              </Avatar.Fallback>
            </Avatar.Root>
            <div class="grid flex-1 text-left text-sm leading-tight">
              <span class="truncate font-medium">{authState.user?.username || 'User'}</span>
              <span class="truncate text-xs">CSFX-Core</span>
            </div>
          </div>
        </DropdownMenu.Label>
        <DropdownMenu.Separator />
        <DropdownMenu.Group>
          <DropdownMenu.Item>
            <BadgeCheckIcon />
            Account
          </DropdownMenu.Item>
          <DropdownMenu.Item>
            <CreditCardIcon />
            Billing
          </DropdownMenu.Item>
          <DropdownMenu.Item>
            <BellIcon />
            Notifications
          </DropdownMenu.Item>
        </DropdownMenu.Group>
        <DropdownMenu.Separator />
        <DropdownMenu.Item onclick={handleLogout}>
          <LogOutIcon />
          Log out
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </Sidebar.MenuItem>
</Sidebar.Menu>
