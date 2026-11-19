<script lang="ts">
    import { HardDrive } from "@lucide/svelte";
    import type { DiskInfo } from "$lib/api/nodes";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { usageTone } from "$lib/utils/dashboard";
    import { formatCapacity } from "$lib/utils/format";
    import { diskUsage, smartTone } from "$lib/utils/node-detail";

    type Props = { hostname: string; disks: DiskInfo[]; loading: boolean; error: string | null };

    let { hostname, disks, loading, error }: Props = $props();

    const totalBytes = $derived(disks.reduce((sum, disk) => sum + disk.total_bytes, 0));
    const usedBytes = $derived(disks.reduce((sum, disk) => sum + disk.used_bytes, 0));

    function deviceName(disk: DiskInfo): string {
        return disk.device.split("/").pop() ?? disk.device;
    }

    function smartLabel(disk: DiskInfo): string {
        return disk.smart ? disk.smart.health.charAt(0).toUpperCase() + disk.smart.health.slice(1).toLowerCase() : "No data";
    }
</script>

{#if error}
    <p class="cx-hint cx-hint--error">{error}</p>
{:else if loading}
    <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
{:else}
    <section class="cx-panel" aria-labelledby="sv-devices">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="sv-devices">Disks</h2>
                <p class="cx-panel__sub">
                    {#if disks.length === 0}
                        Block devices reported by {hostname}
                    {:else}
                        {disks.length} {disks.length === 1 ? "device" : "devices"} on {hostname} &middot; {formatCapacity(usedBytes)} used of {formatCapacity(totalBytes)}
                    {/if}
                </p>
            </div>
        </div>
        {#if disks.length === 0}
            <EmptyState icon={HardDrive} label="No disks detected on this node" />
        {:else}
            <div class="db-scroll">
                <table class="cx-table">
                    <thead>
                        <tr><th>Device</th><th>Model</th><th>Type</th><th>Mount point</th><th>Usage</th><th>Used</th><th>Size</th><th>Health</th></tr>
                    </thead>
                    <tbody>
                        {#each disks as disk (disk.device)}
                            {@const usage = diskUsage(disk)}
                            <tr>
                                <td class="cx-num" style="color: var(--ink)">{deviceName(disk)}</td>
                                <td>{disk.model ?? "—"}</td>
                                <td>{disk.media_type}</td>
                                <td class="cx-num">{disk.mount_point ?? "—"}</td>
                                <td>
                                    <div class="cx-usage {usageTone(usage) ? `cx-usage--${usageTone(usage)}` : ''}">
                                        <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {usage}%"></div></div>
                                        <span class="cx-usage__value">{usage.toFixed(0)}%</span>
                                    </div>
                                </td>
                                <td class="cx-num">{formatCapacity(disk.used_bytes)}</td>
                                <td class="cx-num">{formatCapacity(disk.total_bytes)}</td>
                                <td><span class="cx-badge {smartTone(disk.smart?.health) ? `cx-badge--${smartTone(disk.smart?.health)}` : ''}">{smartLabel(disk)}</span></td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}
    </section>
{/if}
