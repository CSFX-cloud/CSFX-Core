<script lang="ts">
    import type { Workload } from "$lib/api/resource-groups";
    import { formatCapacity } from "$lib/utils/format";

    type Props = { workload: Workload; nodeIp: string | null; groupName: string; groupHref: string };

    let { workload, nodeIp, groupName, groupHref }: Props = $props();

    const sections = $derived([
        {
            id: "general",
            title: "General",
            rows: [
                { label: "ID", value: workload.id, mono: true },
                { label: "Image", value: workload.image, mono: true },
                { label: "Resource group", value: groupName, mono: false, href: groupHref },
                { label: "Created", value: new Date(workload.created_at).toLocaleString(), mono: true },
            ],
        },
        {
            id: "resources",
            title: "Resources",
            rows: [
                { label: "vCPU", value: String(workload.cpu_millicores / 1000), mono: true },
                { label: "Memory", value: formatCapacity(workload.memory_bytes), mono: true },
                { label: "Disk", value: formatCapacity(workload.disk_bytes), mono: true },
            ],
        },
        {
            id: "placement",
            title: "Placement",
            rows: [
                { label: "Node", value: nodeIp ?? workload.assigned_agent_id ?? "—", mono: true },
                { label: "Desired state", value: workload.desired_state, mono: false },
                { label: "Restart policy", value: workload.restart_policy, mono: false },
            ],
        },
    ]);
</script>

<div class="rd-two">
    {#each sections as section (section.id)}
        <section class="cx-panel" aria-labelledby="vm-{section.id}">
            <div class="cx-panel__head"><h2 class="cx-panel__title" id="vm-{section.id}">{section.title}</h2></div>
            <dl class="nd-kv">
                {#each section.rows as row (row.label)}
                    <div>
                        <dt>{row.label}</dt>
                        <dd class:mono={row.mono} style="word-break: break-all">
                            {#if row.href}<a class="cx-link" href={row.href}>{row.value}</a>{:else}{row.value}{/if}
                        </dd>
                    </div>
                {/each}
            </dl>
        </section>
    {/each}
</div>
