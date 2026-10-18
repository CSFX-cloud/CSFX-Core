<script lang="ts" module>
    import BotIcon from "@lucide/svelte/icons/bot";
    import ChartPieIcon from "@lucide/svelte/icons/chart-pie";
    import MapIcon from "@lucide/svelte/icons/map";
    import BookOpenIcon from "@lucide/svelte/icons/book-open";
    import {
        Server,
        Layers,
        ShipWheel,
        Container,
        LaptopMinimal,
        Settings,
    } from "@lucide/svelte";
    import IconBucket from "./icon-bucket.svelte";
    import IconDashboard from "./icon-dashboard.svelte";
    import IconActivity from "./icon-activity.svelte";
    import IconLogs from "./icon-logs.svelte";

    const data = {
        navAdmin: [
            {
                title: "Settings",
                url: "/admin/settings",
                icon: Settings,
            },
        ],
        navBottom: [
            {
                title: "Monitoring",
                url: "/monitoring",
                icon: IconActivity,
            },
            {
                title: "Logs",
                url: "/logs",
                icon: IconLogs,
            },
        ],
        navMain: [
            {
                title: "Dashboard",
                url: "/",
                icon: IconDashboard,
                isActive: true,
            },
            {
                title: "Nodes",
                url: "/nodes",
                icon: Server,
            },
            {
                title: "Resource groups",
                url: "/resource-groups",
                icon: Layers,
            },
        ],
        projects: [
            {
                name: "S3 Buckets",
                url: "/buckets",
                icon: IconBucket,
            },
            {
                name: "Kubernetes",
                url: "#",
                icon: ShipWheel,
            },
            {
                name: "Docker",
                url: "#",
                icon: Container,
            },
            {
                name: "Virtual Machines",
                url: "#",
                icon: LaptopMinimal,
            },
        ],
    };
</script>

<script lang="ts">
    import NavMain from "./nav-main.svelte";
    import NavProjects from "./nav-projects.svelte";
    import NavUser from "./nav-user.svelte";
    import UpdateCard from "./update-card.svelte";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { useSidebar } from "$lib/components/ui/sidebar/context.svelte.js";
    import { auth } from "$lib/auth/store.svelte";
    import { getUpdateStatus } from "$lib/api/system";
    import type { ComponentProps } from "svelte";

    let {
        ref = $bindable(null),
        collapsible = "icon",
        ...restProps
    }: ComponentProps<typeof Sidebar.Root> = $props();

    const sidebar = useSidebar();
    let version = $state<string | null>(null);

    $effect(() => {
        const token = auth.token;
        if (!token) return;
        getUpdateStatus(token)
            .then((status) => {
                version = status.current_version;
            })
            .catch(() => {
                // non-fatal
            });
    });
</script>

<Sidebar.Root bind:ref {collapsible} {...restProps}>
    <Sidebar.Header>
        <button
            onclick={() => sidebar.toggle()}
            class="flex items-center gap-2 px-2 py-1 rounded-md hover:bg-sidebar-accent transition-colors w-full text-left cursor-pointer"
        >
            <img
                src="/logo/logo-csfx.svg"
                alt="CSFX"
                class="size-6 invert dark:invert-0 shrink-0"
            />
            <div
                class="flex flex-col leading-tight group-data-[collapsible=icon]:hidden"
            >
                <span class="font-semibold text-sm tracking-wide">CSFX</span>
                <span class="text-muted-foreground text-xs">
                    {version ? `v${version}` : "Hypervisor"}
                </span>
            </div>
        </button>
    </Sidebar.Header>
    <Sidebar.Content>
        <NavMain items={data.navMain} />
        <Sidebar.Separator />
        <NavProjects projects={data.projects} />
        <NavMain items={data.navBottom} label="" class="mt-auto" />
        <Sidebar.Separator />
        <NavMain items={data.navAdmin} label="Admin" />
    </Sidebar.Content>
    <UpdateCard />
    <Sidebar.Footer>
        <NavUser />
    </Sidebar.Footer>
    <Sidebar.Rail />
</Sidebar.Root>
