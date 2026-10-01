<script lang="ts">
    import type { ClusterStats } from "$lib/api/nodes";

    let { stats }: { stats: ClusterStats | null } = $props();

    function formatBytes(bytes: number | null | undefined): string {
        if (bytes == null) return "-";
        const gb = bytes / 1_073_741_824;
        if (gb >= 1024) return (gb / 1024).toFixed(1) + " TB";
        return gb.toFixed(1) + " GB";
    }

    const cpuPercent = $derived(stats?.avg_cpu_usage_percent ?? 0);
</script>

{#snippet statRow(label: string, value: string, hint?: string)}
    <div class="flex items-baseline justify-between gap-2 py-2 border-b border-border last:border-b-0">
        <span class="text-xs text-muted-foreground">{label}</span>
        <span class="text-sm font-medium tabular-nums">
            {value}
            {#if hint}
                <span class="text-xs text-muted-foreground font-normal">{hint}</span>
            {/if}
        </span>
    </div>
{/snippet}

<div class="flex h-full flex-col overflow-y-auto">
    <div class="relative h-48 shrink-0 overflow-hidden">
        <img
            src="/server/server_rack_3x.png"
            alt=""
            class="absolute inset-0 size-full object-cover"
            style="object-position: center 0%;"
        />
        <div
            class="absolute inset-0"
            style="background: linear-gradient(to bottom, transparent 12%, var(--background) 90%);"
        ></div>
        <span class="absolute inset-x-4 bottom-3 z-10 text-label uppercase text-muted-foreground tracking-[0.02em]">Cluster overview</span>
    </div>

    <div class="flex flex-col px-4 pt-4 pb-4">
    {#if stats}
        {@render statRow("Total nodes", String(stats.node_count))}
        {@render statRow("Online", `${stats.online_count} / ${stats.node_count}`)}
        {@render statRow("vCPU capacity", String(stats.total_cpu_cores), `${cpuPercent.toFixed(1)}% used`)}
        {@render statRow("Memory", formatBytes(stats.used_memory_bytes), `/ ${formatBytes(stats.total_memory_bytes)}`)}
        {@render statRow("Disk", formatBytes(stats.used_disk_bytes), `/ ${formatBytes(stats.total_disk_bytes)}`)}
    {/if}
    </div>
</div>
