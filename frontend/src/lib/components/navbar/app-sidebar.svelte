<script lang="ts" module>
  import {
    HandCoins,
    RepeatIcon,
    ChartPieIcon,
    LayoutDashboardIcon,
    PiggyBank,
    ReceiptText,
    Users,
    FolderKanban,
    FileText,
    Server,
    FolderOpen,
    ShoppingBag,
    Package,
  } from '@lucide/svelte';

  // Data for FinanceVault app
  const data = {
    user: {
      name: 'Max Mustermann',
      email: 'max@example.com',
      avatar: '/avatars/user.jpg',
    },
    teams: [
      {
        name: 'CSFX Core',
        plan: 'Premium',
      },
    ],
    navMain: [
      {
        title: 'Dashboard',
        url: '/',
        icon: LayoutDashboardIcon,
        isActive: true,
      },
      {
        title: 'Physical Servers',
        url: '/physical-servers',
        icon: Server,
      },
      {
        title: 'Resource Groups',
        url: '/resource-groups',
        icon: FolderOpen,
      },
      {
        title: 'Resources',
        url: '/resources',
        icon: Package,
      },
      {
        title: 'Marketplace',
        url: '/marketplace',
        icon: ShoppingBag,
      },
      {
        title: 'Customers',
        url: '/customers',
        icon: Users,
      },
      {
        title: 'Projects',
        url: '/projects',
        icon: FolderKanban,
      },
      {
        title: 'Documents',
        url: '/documents',
        icon: FileText,
      },
      {
        title: 'Reports',
        url: '/reports',
        icon: ChartPieIcon,
      },
    ],
    favorites: [
      {
        name: 'Max Mustermann',
        company: 'Tech Solutions GmbH',
        url: '/customers/max-mustermann',
        initials: 'M',
      },
      {
        name: 'Anna Schmidt',
        company: 'Digital Marketing AG',
        url: '/customers/anna-schmidt',
        initials: 'A',
      },
      {
        name: 'Stefan Klein',
        company: 'Klein & Partners',
        url: '/customers/stefan-klein',
        initials: 'S',
      },
    ],
  };
</script>

<script lang="ts">
  import NavMain from './nav-main.svelte';
  import NavFavorites from './nav-favorites.svelte';
  import NavProjects from './nav-projects.svelte';
  import NavUser from './nav-user.svelte';
  import TeamSwitcher from './team-switcher.svelte';
  import UpdateNotification from '$lib/components/UpdateNotification.svelte';
  import * as Sidebar from '$lib/components/ui/sidebar/index.js';
  import type { ComponentProps } from 'svelte';

  let {
    ref = $bindable(null),
    collapsible = 'icon',
    ...restProps
  }: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root {collapsible} {...restProps}>
  <Sidebar.Header>
    <TeamSwitcher />
  </Sidebar.Header>
  <Sidebar.Content>
    <NavMain items={data.navMain} />
    <NavFavorites favorites={data.favorites} />
  </Sidebar.Content>
  <UpdateNotification />
  <Sidebar.Footer>
    <NavUser />
  </Sidebar.Footer>
  <Sidebar.Rail />
</Sidebar.Root>
