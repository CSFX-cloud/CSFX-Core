<script lang="ts">
    import type { Workload } from "$lib/api/resource-groups";
    import { fmtBytes } from "$lib/utils/format";

    let { workload, nodeIp }: { workload: Workload; nodeIp: string | null } = $props();

    const rows = $derived([
        { label: "ID", value: workload.id },
        { label: "Image", value: workload.image },
        { label: "vCPU", value: String(workload.cpu_millicores / 1000) },
        { label: "Memory", value: fmtBytes(workload.memory_bytes) },
        { label: "Disk", value: fmtBytes(workload.disk_bytes) },
        { label: "Node", value: nodeIp ?? workload.assigned_agent_id ?? "-" },
        { label: "Desired state", value: workload.desired_state },
        { label: "Restart policy", value: workload.restart_policy },
        { label: "Created", value: new Date(workload.created_at).toLocaleString() },
    ]);
</script>

<div class="px-6 py-4">
    <div class="border rounded-lg divide-y">
        {#each rows as row (row.label)}
            <div class="flex items-center justify-between gap-4 px-4 py-2.5 text-sm">
                <span class="text-muted-foreground shrink-0">{row.label}</span>
                <span class="font-mono text-xs truncate">{row.value}</span>
            </div>
        {/each}
    </div>
</div>
