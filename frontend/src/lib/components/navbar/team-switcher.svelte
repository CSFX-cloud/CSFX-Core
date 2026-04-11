<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar/index.js';
  import VaultIcon from '@lucide/svelte/icons/vault';
  import { useSidebar } from '$lib/components/ui/sidebar/index.js';

  const sidebar = useSidebar();

  function handleClick() {
    sidebar.toggle();
  }
</script>

<Sidebar.Menu>
  <Sidebar.MenuItem>
    <Sidebar.MenuButton size="lg" onclick={handleClick}>
      <div
        class="flex aspect-square size-10 items-center justify-center rounded-lg overflow-hidden"
      >
        <img
          src="/logos/CSF_Logo.png"
          alt="CSFX-Core Logo"
          class="w-full h-full object-contain rounded-lg max-w-8 max-h-8"
        />
      </div>
      <div class="grid flex-1 text-left text-sm leading-tight group-data-[collapsible=icon]:hidden">
        <span class="truncate font-semibold text-lg"> CSFX-Core </span>
        <span class="truncate text-xs text-muted-foreground"> Business Platform </span>
      </div>
    </Sidebar.MenuButton>
  </Sidebar.MenuItem>
</Sidebar.Menu>
