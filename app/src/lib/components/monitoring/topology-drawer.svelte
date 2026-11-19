<script lang="ts">
    import { Box, HardDrive, Monitor, Server } from "@lucide/svelte";
    import type { Node } from "@xyflow/svelte";
    import type { LogsFilter } from "$lib/api/logs";
    import TopologyLogs from "$lib/components/monitoring/topology-logs.svelte";
    import SideDrawer from "$lib/components/shell/side-drawer.svelte";
    import { nodeTone, usageTone } from "$lib/utils/dashboard";
    import { formatBytes } from "$lib/utils/format";
    import { statusLabel, toneBadgeClass } from "$lib/utils/tone";

    const LOG_LIMIT = 8;

    type Row = { label: string; value: string; mono?: boolean };
    type Usage = { label: string; percent: number };
    type Action = { label: string; href: string; primary?: boolean };

    let { node, groupNames, onclose }: { node: Node | null; groupNames: Map<string, string>; onclose: () => void } = $props();

    const data = $derived(node?.data ?? {});
    const kind = $derived(node?.type ?? "");
    const groupId = $derived(typeof data.resource_group_id === "string" ? data.resource_group_id : "");
    const groupName = $derived(groupNames.get(groupId) ?? groupId);
    const status = $derived(typeof data.status === "string" ? data.status : "");
    const isVm = $derived(data.runtime_class === "vm");

    const Icon = $derived(kind === "agent" ? Server : kind === "volume" ? HardDrive : isVm ? Monitor : Box);

    const subtitle = $derived(
        kind === "agent" ? `${data.role} node` : kind === "volume" ? "Block storage volume" : kind === "workload" ? (isVm ? "Virtual machine" : "Container") : "",
    );

    const usages = $derived<Usage[]>(
        kind === "agent"
            ? [
                  { label: "CPU", percent: Number(data.cpu_percent) },
                  { label: "Memory", percent: Number(data.memory_percent) },
                  { label: "Disk", percent: Number(data.disk_percent) },
              ].filter((usage) => Number.isFinite(usage.percent))
            : [],
    );

    const rows = $derived<Row[]>(buildRows());
    const actions = $derived<Action[]>(buildActions());
    const logFilter = $derived<LogsFilter | null>(buildLogFilter());

    function percentOf(value: unknown): string {
        return typeof value === "number" ? `${value.toFixed(1)}%` : "—";
    }

    function buildRows(): Row[] {
        if (kind === "agent") {
            return [
                { label: "IP address", value: String(data.ip_address ?? "—"), mono: true },
                { label: "Operating system", value: String(data.os_type ?? "—") },
                { label: "Agent version", value: String(data.agent_version ?? "—"), mono: true },
                { label: "Scheduling", value: data.cordoned ? "Cordoned" : "Schedulable" },
                { label: "Node ID", value: String(data.id), mono: true },
            ];
        }
        if (kind === "workload") {
            return [
                { label: "Image", value: String(data.image ?? "—"), mono: true },
                { label: "Resource group", value: groupName || "—" },
                { label: "CPU", value: percentOf(data.cpu_percent), mono: true },
                { label: "Memory", value: typeof data.memory_bytes === "number" ? formatBytes(data.memory_bytes) : "—", mono: true },
                { label: "Workload ID", value: String(data.id), mono: true },
            ];
        }
        if (kind === "volume") {
            return [
                { label: "Size", value: `${data.size_gb} GB`, mono: true },
                { label: "Resource group", value: groupName || "—" },
                { label: "Volume ID", value: String(data.id), mono: true },
            ];
        }
        return [];
    }

    function buildLogFilter(): LogsFilter | null {
        if (kind === "agent") return { agent_id: String(data.id), limit: LOG_LIMIT };
        return kind === "workload" ? { workload_id: String(data.id), limit: LOG_LIMIT } : null;
    }

    function buildActions(): Action[] {
        if (kind === "agent") {
            return [
                { label: "Node details", href: `/nodes/${data.id}`, primary: true },
                { label: "Node logs", href: `/logs?node=${data.id}` },
            ];
        }
        if (!groupId) return [];
        const base = `/resource-groups/${groupId}`;
        if (kind === "workload") return [{ label: isVm ? "Open virtual machine" : "Open container", href: `${base}/${isVm ? "vm" : "container"}/${data.id}`, primary: true }];
        return kind === "volume" ? [{ label: "Open resource group", href: base, primary: true }] : [];
    }
</script>

<SideDrawer open={node !== null} title={String(data.label ?? "")} {subtitle} {onclose}>
    {#snippet header()}
        <span class="cx-tile-plain nl-tile" aria-hidden="true"><Icon class="cx-i" /></span>
    {/snippet}
    {#if node}
        {#if kind === "unassigned"}
            <p class="cx-hint">Workloads that are waiting for a node to run on appear here.</p>
        {:else}
            {#if status}
                <div>
                    <span class={toneBadgeClass(kind === "agent" ? nodeTone(status) : "")}>{statusLabel(status)}</span>
                </div>
            {/if}
            {#if usages.length > 0}
                <dl class="nd-kv">
                    {#each usages as usage (usage.label)}
                        <div>
                            <dt>{usage.label}</dt>
                            <dd>
                                <div class="cx-usage {usageTone(usage.percent) ? `cx-usage--${usageTone(usage.percent)}` : ''}">
                                    <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {usage.percent}%"></div></div>
                                    <span class="cx-usage__value">{usage.percent.toFixed(0)}%</span>
                                </div>
                            </dd>
                        </div>
                    {/each}
                </dl>
            {/if}
            <dl class="nd-kv">
                {#each rows as row (row.label)}
                    <div><dt>{row.label}</dt><dd class:mono={row.mono}>{row.value}</dd></div>
                {/each}
            </dl>
            {#if actions.length > 0}
                <div class="db-actions">
                    {#each actions as action (action.href)}
                        <a class="cx-btn {action.primary ? 'cx-btn--primary' : 'cx-btn--secondary'} cx-btn--sm" href={action.href}>{action.label}</a>
                    {/each}
                </div>
            {/if}
            {#if logFilter}
                {#key node.id}
                    <TopologyLogs filter={logFilter} href={kind === "agent" ? `/logs?node=${data.id}` : undefined} />
                {/key}
            {/if}
        {/if}
    {/if}
</SideDrawer>
