<script lang="ts">
  import '../app.css';
  import favicon from '$lib/assets/favicon.svg';
  import { onMount, onDestroy } from 'svelte';
  import AppSidebar from '$lib/components/navbar/app-sidebar.svelte';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import * as Sidebar from '$lib/components/ui/sidebar/index.js';
  import { theme, effectiveTheme, initThemeFromStorage } from '$lib/stores/theme';
  import { page } from '$app/stores';
  import { authStore } from '$lib/stores/auth';
  import { ApiClient } from '$lib/services/api-client';

  let { data, children } = $props();

  // Check if we should show the sidebar (not on signin/signup pages)
  let showSidebar = $derived(
    !$page.url.pathname.startsWith('/signin') &&
      !$page.url.pathname.startsWith('/signup') &&
      !$page.url.pathname.startsWith('/otp')
  );

  // On the client we initialize the store from localStorage and
  // subscribe to the effective theme to keep the <html> class in sync.
  onMount(() => {
    console.log('[+layout.svelte] Component mounted.');
    console.log('[+layout.svelte] Data received from load function:', data);

    // Initialize auth store with user from server
    authStore.init(data.user, data.token);

    // If no user and not on public pages, redirect to signin
    const publicPages = ['/signin', '/signup', '/otp'];
    const currentPath = $page.url.pathname;
    if (!data.user && !publicPages.some((p) => currentPath.startsWith(p))) {
      console.log('[+layout.svelte] No user found, redirecting to signin');
      window.location.href = '/signin';
      return;
    }

    // Initialize theme
    initThemeFromStorage();

    const unsub = effectiveTheme.subscribe((t) => {
      try {
        if (t === 'dark') document.documentElement.classList.add('dark');
        else document.documentElement.classList.remove('dark');
        // reflect chosen preference for debugging/other CSS
        try {
          document.documentElement.setAttribute('data-theme', t === 'dark' ? 'dark' : 'light');
        } catch (e) {}
      } catch (e) {
        // ignore
      }
    });

    return unsub;
  });
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
</svelte:head>

{#if showSidebar}
  <Sidebar.Provider>
    <AppSidebar />
    <Sidebar.Inset>
      <div class="flex flex-1 flex-col gap-4 p-4 pt-0">
        {@render children?.()}
      </div>
    </Sidebar.Inset>
  </Sidebar.Provider>
{:else}
  <!-- Content without sidebar for signin/signup pages -->
  {@render children?.()}
{/if}
