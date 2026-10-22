<script lang="ts">
    import { Disc2, HardDrive, Plus } from "@lucide/svelte";
    import type { DiskInfo } from "$lib/api/nodes";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { usageTone } from "$lib/utils/dashboard";
    import { formatCapacity } from "$lib/utils/format";
    import { bayCount, bayLabel, diskUsage, isOptical, smartTone } from "$lib/utils/node-detail";

    type Props = { hostname: string; disks: DiskInfo[]; loading: boolean; error: string | null };

    let { hostname, disks, loading, error }: Props = $props();

    let selected = $state(0);

    const bays = $derived(Array.from({ length: bayCount(disks.length) }, (_, index) => ({ index, disk: disks[index] ?? null })));
    const current = $derived(bays[selected] ?? bays[0]);
    const inUse = $derived(disks.filter((disk) => disk.used_bytes > 0).length);

    function deviceName(disk: DiskInfo): string {
        return disk.device.split("/").pop() ?? disk.device;
    }

    function bayClass(disk: DiskInfo | null): string {
        if (!disk) return "sv-bay--empty";
        return isOptical(disk) ? "sv-bay--optical" : "sv-bay--disk";
    }

    function smartLabel(disk: DiskInfo): string {
        return disk.smart ? disk.smart.health.charAt(0).toUpperCase() + disk.smart.health.slice(1).toLowerCase() : "No data";
    }
</script>

{#snippet smartBadge(disk: DiskInfo)}
    <span class="cx-badge {smartTone(disk.smart?.health) ? `cx-badge--${smartTone(disk.smart?.health)}` : ''}">{smartLabel(disk)}</span>
{/snippet}

{#if error}
    <p class="cx-hint cx-hint--error">{error}</p>
{:else if loading}
    <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
{:else}
    <div style="display: grid; gap: 12px">
        <div class="nd-grid">
            <section class="cx-stage sv-stage" aria-label="Drive bays">
                <div class="cx-stage__canvas"></div>
                <div class="sv-wrap">
                    <div class="sv-caption">
                        <span>/Front panel</span>
                        <span>{bays.length} bays &middot; {inUse} in use</span>
                    </div>

                    <div class="sv-chassis">
                        <div class="sv-ear" aria-hidden="true">
                            <span class="sv-handle"></span>
                            <span class="sv-led sv-led--on"></span>
                            <span class="sv-ear__label">PWR</span>
                        </div>

                        <div class="sv-bays" role="group" aria-label="Select a drive bay">
                            {#each bays as bay (bay.index)}
                                <button
                                    class="sv-bay {bayClass(bay.disk)} {selected === bay.index ? 'is-sel' : ''}"
                                    type="button"
                                    aria-pressed={selected === bay.index}
                                    onclick={() => (selected = bay.index)}
                                >
                                    <span class="sv-bay__top">
                                        <span class="sv-led {bay.disk ? (bay.disk.used_bytes > 0 ? 'sv-led--on' : '') : 'sv-led--off'}"></span>
                                        <span>{bayLabel(bay.index)}</span>
                                    </span>
                                    {#if bay.disk}
                                        <span class="sv-bay__name">{deviceName(bay.disk)}</span>
                                        {#if isOptical(bay.disk)}
                                            <span class="sv-slit"></span>
                                        {:else}
                                            <span class="sv-bay__bar"><span style="width: {diskUsage(bay.disk)}%"></span></span>
                                        {/if}
                                    {:else}
                                        <span class="sv-vent"></span>
                                    {/if}
                                </button>
                            {/each}
                        </div>

                        <div class="sv-ear sv-ear--r" aria-hidden="true">
                            <span class="sv-ear__label">{hostname}</span>
                            <span class="sv-port"></span>
                            <span class="sv-handle"></span>
                        </div>
                    </div>

                    <div class="sv-legend">
                        <span><i class="sv-led sv-led--on"></i>Active</span>
                        <span><i class="sv-led"></i>Idle</span>
                        <span><i class="sv-led sv-led--off"></i>Empty</span>
                    </div>
                </div>
            </section>

            <section class="cx-panel sv-detail" aria-live="polite">
                {#if current.disk}
                    {@const disk = current.disk}
                    {@const usage = diskUsage(disk)}
                    <div class="sv-detail__in">
                        <div class="nd-disk__head">
                            <span class="nd-tile nd-tile--sm" aria-hidden="true">
                                {#if isOptical(disk)}<Disc2 class="cx-i" />{:else}<HardDrive class="cx-i" />{/if}
                            </span>
                            <div class="nd-disk__name">
                                <h3>{disk.model ?? disk.device}</h3>
                                <p>Bay {bayLabel(current.index)} &middot; {disk.device}</p>
                            </div>
                            <span class="cx-badge {disk.used_bytes > 0 ? 'cx-badge--success' : ''}">{disk.used_bytes > 0 ? "Active" : "Idle"}</span>
                        </div>
                        <div class="sv-big">
                            <p class="cx-stat__value stat">{formatCapacity(disk.used_bytes)} <small>/ {formatCapacity(disk.total_bytes)}</small></p>
                            <div class="cx-usage {usageTone(usage) ? `cx-usage--${usageTone(usage)}` : ''}">
                                <div class="cx-usage__track" style="max-width: none"><div class="cx-usage__fill" style="width: {usage}%"></div></div>
                                <span class="cx-usage__value">{usage.toFixed(0)}%</span>
                            </div>
                        </div>
                        <dl class="nd-kv">
                            <div><dt>Type</dt><dd>{disk.media_type}</dd></div>
                            <div><dt>Mount point</dt>{#if disk.mount_point}<dd class="mono">{disk.mount_point}</dd>{:else}<dd class="na">Not mounted</dd>{/if}</div>
                            <div><dt>Free</dt><dd class="mono">{formatCapacity(Math.max(0, disk.total_bytes - disk.used_bytes))}</dd></div>
                            <div><dt>SMART</dt><dd>{@render smartBadge(disk)}</dd></div>
                            {#if disk.smart?.power_on_hours != null}
                                <div><dt>Power-on hours</dt><dd class="mono">{disk.smart.power_on_hours.toLocaleString()} h</dd></div>
                            {/if}
                            {#if disk.smart?.temperature_celsius != null}
                                <div><dt>Temperature</dt><dd class="mono">{disk.smart.temperature_celsius} &deg;C</dd></div>
                            {/if}
                        </dl>
                    </div>
                {:else}
                    <div class="sv-detail__in sv-detail__empty">
                        <span class="cx-tile-plain" aria-hidden="true" style="width: 48px; height: 48px"><Plus class="cx-i" /></span>
                        <h3 class="nd-empty__title nd-empty__title--sm">Bay {bayLabel(current.index)} is empty</h3>
                        <p class="nd-empty__text nd-empty__text--sm">Insert a disk and it shows up here after the next heartbeat.</p>
                    </div>
                {/if}
            </section>
        </div>

        <section class="cx-panel" aria-labelledby="sv-devices">
            <div class="cx-panel__head">
                <div>
                    <h2 class="cx-panel__title" id="sv-devices">Devices</h2>
                    <p class="cx-panel__sub">Block devices reported by {hostname}</p>
                </div>
            </div>
            {#if disks.length === 0}
                <EmptyState icon={HardDrive} label="No disks detected on this node" />
            {:else}
                <div class="db-scroll">
                    <table class="cx-table">
                        <thead><tr><th>Bay</th><th>Device</th><th>Model</th><th>Mount</th><th>Usage</th><th>Size</th><th>SMART</th></tr></thead>
                        <tbody>
                            {#each disks as disk, index (disk.device)}
                                {@const usage = diskUsage(disk)}
                                <tr>
                                    <td class="cx-num">{bayLabel(index)}</td>
                                    <td class="cx-num" style="color: var(--ink)">{deviceName(disk)}</td>
                                    <td>{disk.model ?? "—"}</td>
                                    <td class="cx-num">{disk.mount_point ?? "—"}</td>
                                    <td>
                                        <div class="cx-usage {usageTone(usage) ? `cx-usage--${usageTone(usage)}` : ''}">
                                            <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {usage}%"></div></div>
                                            <span class="cx-usage__value">{usage.toFixed(0)}%</span>
                                        </div>
                                    </td>
                                    <td class="cx-num">{formatCapacity(disk.total_bytes)}</td>
                                    <td>{@render smartBadge(disk)}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            {/if}
        </section>
    </div>
{/if}
