<script lang="ts">
    import { goto } from '$app/navigation';
    import { Command } from 'bits-ui';
    import * as Dialog from '$lib/components/ui/dialog/index.js';
    import { auth } from '$lib/auth/store.svelte';
    import { commandPalette } from './command-palette-store.svelte.js';
    import { listNodes, type Node } from '$lib/api/nodes';
    import { listWorkloads, listResourceGroups, listBuckets, type Workload, type ResourceGroup, type Bucket } from '$lib/api/resource-groups';
    import { listEvents, type AlertEvent } from '$lib/api/events';
    import ServerIcon from '@lucide/svelte/icons/server';
    import LayersIcon from '@lucide/svelte/icons/layers';
    import BoxIcon from '@lucide/svelte/icons/box';
    import MonitorIcon from '@lucide/svelte/icons/monitor';
    import BucketIcon from '@lucide/svelte/icons/database';
    import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
    import SettingsIcon from '@lucide/svelte/icons/settings';
    import GaugeIcon from '@lucide/svelte/icons/gauge';
    import ScrollTextIcon from '@lucide/svelte/icons/scroll-text';

    type ResultItem = {
        id: string;
        label: string;
        sublabel?: string;
        icon: typeof ServerIcon;
        group: string;
        go: () => void;
    };

    const pages: ResultItem[] = [
        { id: 'page-dashboard', label: 'Dashboard', icon: GaugeIcon, group: 'Pages', go: () => goto('/') },
        { id: 'page-nodes', label: 'Nodes', icon: ServerIcon, group: 'Pages', go: () => goto('/nodes') },
        { id: 'page-resource-groups', label: 'Resource Groups', icon: LayersIcon, group: 'Pages', go: () => goto('/resource-groups') },
        { id: 'page-buckets', label: 'S3 Buckets', icon: BucketIcon, group: 'Pages', go: () => goto('/buckets') },
        { id: 'page-logs', label: 'Logs', icon: ScrollTextIcon, group: 'Pages', go: () => goto('/logs') },
        { id: 'page-settings', label: 'Settings', icon: SettingsIcon, group: 'Pages', go: () => goto('/admin/settings') },
    ];

    let loading = $state(false);
    let nodes = $state<Node[]>([]);
    let workloads = $state<Workload[]>([]);
    let resourceGroups = $state<ResourceGroup[]>([]);
    let buckets = $state<Bucket[]>([]);
    let alerts = $state<AlertEvent[]>([]);

    async function loadData() {
        if (!auth.token || loading) return;
        loading = true;
        try {
            const [nodesRes, workloadsRes, rgRes, bucketsRes, alertsRes] = await Promise.all([
                listNodes(auth.token),
                listWorkloads(auth.token),
                listResourceGroups(auth.token),
                listBuckets(auth.token),
                listEvents(auth.token, { status: 'open' }),
            ]);
            nodes = nodesRes;
            workloads = workloadsRes;
            resourceGroups = rgRes;
            buckets = bucketsRes;
            alerts = alertsRes;
        } catch {
            // best-effort, palette still works with pages
        } finally {
            loading = false;
        }
    }

    $effect(() => {
        if (commandPalette.open) loadData();
    });

    function rgNameById(id: string | null): string | undefined {
        return resourceGroups.find((rg) => rg.id === id)?.name;
    }

    let nodeResults = $derived<ResultItem[]>(
        nodes.map((n) => ({
            id: `node-${n.id}`,
            label: n.hostname,
            sublabel: n.ip_address ?? undefined,
            icon: ServerIcon,
            group: 'Nodes',
            go: () => goto(`/nodes/${n.id}`),
        })),
    );

    let workloadResults = $derived<ResultItem[]>(
        workloads.map((w) => ({
            id: `workload-${w.id}`,
            label: w.name,
            sublabel: w.image,
            icon: w.runtime_class === 'vm' ? MonitorIcon : BoxIcon,
            group: 'Workloads',
            go: () =>
                w.runtime_class === 'vm' && w.resource_group_id
                    ? goto(`/resource-groups/${w.resource_group_id}/vm/${w.id}`)
                    : goto(`/resource-groups/${w.resource_group_id}`),
        })),
    );

    let resourceGroupResults = $derived<ResultItem[]>(
        resourceGroups.map((rg) => ({
            id: `rg-${rg.id}`,
            label: rg.name,
            sublabel: rg.internal_cidr,
            icon: LayersIcon,
            group: 'Resource Groups',
            go: () => goto(`/resource-groups/${rg.id}`),
        })),
    );

    let bucketResults = $derived<ResultItem[]>(
        buckets.map((b) => ({
            id: `bucket-${b.id}`,
            label: b.name,
            sublabel: b.exposure,
            icon: BucketIcon,
            group: 'Buckets',
            go: () => goto(`/buckets/${b.id}`),
        })),
    );

    let alertResults = $derived<ResultItem[]>(
        alerts.map((a) => {
            const node = nodes.find((n) => n.id === a.agent_id);
            return {
                id: `alert-${a.id}`,
                label: a.message ?? a.event_type,
                sublabel: node?.hostname,
                icon: TriangleAlertIcon,
                group: 'Alerts',
                go: () => (node ? goto(`/nodes/${node.id}?tab=alerts`) : undefined),
            };
        }),
    );

    let groupedResults = $derived<Record<string, ResultItem[]>>({
        Pages: pages,
        Nodes: nodeResults,
        Workloads: workloadResults,
        'Resource Groups': resourceGroupResults,
        Buckets: bucketResults,
        Alerts: alertResults,
    });

    function handleSelect(item: ResultItem) {
        commandPalette.hide();
        item.go();
    }

    function handleKeydown(e: KeyboardEvent) {
        if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
            e.preventDefault();
            commandPalette.toggle();
        }
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<Dialog.Root open={commandPalette.open} onOpenChange={(value) => commandPalette.set(value)}>
    <Dialog.Content class="w-[min(90vw,560px)] p-0 top-[20%] translate-y-0" showCloseButton={false}>
        <Command.Root class="flex flex-col overflow-hidden" loop>
            <Command.Input
                placeholder="Search nodes, workloads, alerts, settings..."
                class="w-full border-b px-4 py-3 text-sm bg-transparent outline-none placeholder:text-muted-foreground"
            />
            <Command.List class="max-h-[60vh] overflow-y-auto p-2">
                <Command.Empty class="py-8 text-center text-sm text-muted-foreground">
                    {loading ? 'Loading...' : 'No results found.'}
                </Command.Empty>
                {#each Object.entries(groupedResults) as [groupName, items] (groupName)}
                    {#if items.length > 0}
                        <Command.Group>
                            <Command.GroupHeading class="px-2 py-1.5 text-xs font-medium text-muted-foreground">
                                {groupName}
                            </Command.GroupHeading>
                            <Command.GroupItems>
                                {#each items as item (item.id)}
                                    <Command.Item
                                        keywords={item.sublabel ? [item.sublabel] : undefined}
                                        onSelect={() => handleSelect(item)}
                                        class="flex items-center gap-2.5 px-2.5 py-2 rounded-md text-sm cursor-pointer data-selected:bg-muted"
                                    >
                                        <item.icon class="size-4 text-muted-foreground shrink-0" />
                                        <span class="flex-1 min-w-0 truncate">{item.label}</span>
                                        {#if item.sublabel}
                                            <span class="text-xs text-muted-foreground truncate max-w-[40%]">{item.sublabel}</span>
                                        {/if}
                                    </Command.Item>
                                {/each}
                            </Command.GroupItems>
                        </Command.Group>
                    {/if}
                {/each}
            </Command.List>
        </Command.Root>
    </Dialog.Content>
</Dialog.Root>
