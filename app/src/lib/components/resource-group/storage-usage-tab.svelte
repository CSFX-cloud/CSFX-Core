<script lang="ts">
    import { Maximize2, Plus } from "@lucide/svelte";
    import type { Volume, Workload } from "$lib/api/resource-groups";
    import { formatBytes } from "$lib/utils/format";
    import { usageSegments, usedPercent, type StorageRow } from "$lib/utils/storage";
    import { badgeClass, statusLabel } from "$lib/utils/tone";
    import AttachVolumeDialog from "./attach-volume-dialog.svelte";
    import ResizeVolumeDialog from "./resize-volume-dialog.svelte";

    const PERCENT = 100;

    type Props = {
        rows: StorageRow[];
        title: string;
        rgId: string;
        workload?: Workload;
        available: Volume[];
        restart: () => Promise<unknown>;
        onChanged: () => Promise<void>;
    };

    let { rows, title, rgId, workload, available, restart, onChanged }: Props = $props();

    let attachDialog = $state<AttachVolumeDialog | null>(null);
    let resizeDialog = $state<ResizeVolumeDialog | null>(null);

    const measured = $derived(rows.filter((row) => row.usedBytes !== null));
    const used = $derived(measured.reduce((sum, row) => sum + (row.usedBytes ?? 0), 0));
    const total = $derived(rows.reduce((sum, row) => sum + row.totalBytes, 0));

    const width = (bytes: number, row: StorageRow) => `${(bytes / row.totalBytes) * PERCENT}%`;

    async function resized() {
        await restart();
        await onChanged();
    }
</script>

<section class="cx-panel" aria-labelledby="su-title">
    <div class="cx-panel__head">
        <div><h2 class="cx-panel__title" id="su-title">Storage</h2><p class="cx-panel__sub">Volumes used by {title}</p></div>
        <div class="su-actions">
            {#if rows.length}
                <span class="cx-num">{measured.length ? `${formatBytes(used)} of ${formatBytes(total)} used` : `${formatBytes(total)} provisioned`}</span>
            {/if}
            {#if workload}
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => attachDialog?.open()}>
                    <Plus class="cx-i" aria-hidden="true" />Add volume
                </button>
            {/if}
        </div>
    </div>
    {#if rows.length === 0}
        <p class="cx-hint">No volumes attached.</p>
    {:else}
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Volume</th><th>Mount</th><th>Used</th><th>Status</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
                <tbody>
                    {#each rows as row (`${row.volume.id}:${row.mountPath}`)}
                        {@const parts = usageSegments(row)}
                        {@const percent = usedPercent(row.usedBytes, row.totalBytes)}
                        <tr>
                            <td><b>{row.volume.name}</b> <span class="cx-soft">&middot; {row.volume.pool}</span></td>
                            <td class="cx-num">{row.mountPath}</td>
                            <td class="su-used">
                                {#if percent === null}
                                    <span class="cx-soft">&mdash; of {formatBytes(row.totalBytes)}</span>
                                {:else}
                                    <div class="su-bar" role="img" aria-label="{percent.toFixed(0)}% of {row.volume.name} used">
                                        {#each parts as part (part.name)}
                                            <span style="width: {width(part.bytes, row)}; background: {part.color}" title="{part.name} · {formatBytes(part.bytes)}"></span>
                                        {/each}
                                    </div>
                                    <span class="cx-soft">{formatBytes(row.usedBytes)} of {formatBytes(row.totalBytes)} &middot; {formatBytes(row.totalBytes - (row.usedBytes ?? 0))} free</span>
                                {/if}
                            </td>
                            <td><span class={badgeClass(row.volume.status)}>{statusLabel(row.volume.status)}</span></td>
                            <td>
                                <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Grow {row.volume.name}" title="Grow volume" onclick={() => resizeDialog?.open(row.volume)}>
                                    <Maximize2 class="cx-i" aria-hidden="true" />
                                </button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</section>

{#if workload}
    <AttachVolumeDialog bind:this={attachDialog} {workload} {rgId} {available} onAttached={onChanged} />
{/if}
<ResizeVolumeDialog bind:this={resizeDialog} onResized={resized} />

<style>
    .su-actions {
        display: flex;
        align-items: center;
        gap: var(--space-3);
    }
    .su-used {
        display: grid;
        gap: var(--space-2);
        min-width: 220px;
    }
    .su-bar {
        display: flex;
        height: 8px;
        border-radius: var(--radius-pill);
        overflow: hidden;
        background: var(--bg-300);
    }
    .su-bar span {
        border-right: 1px solid var(--bg-100);
    }
</style>
