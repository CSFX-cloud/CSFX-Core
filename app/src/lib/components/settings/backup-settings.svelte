<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { Plus } from "@lucide/svelte";
    import {
        deleteExportTarget,
        listExports,
        listExportTargets,
        testExportTarget,
        type ExportRecord,
        type ExportTarget,
    } from "$lib/api/exports";
    import { formatBytes, pluralize } from "$lib/utils/format";
    import { RESTORABLE_SCOPES, scopeLabel, statusBadge } from "$lib/utils/exports";
    import ExportDialog from "$lib/components/settings/export-dialog.svelte";
    import ExportTargetDialog from "$lib/components/settings/export-target-dialog.svelte";
    import RestoreDialog from "$lib/components/settings/restore-dialog.svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";

    const POLL_INTERVAL_MS = 2000;

    let targets = $state<ExportTarget[]>([]);
    let exports = $state<ExportRecord[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);
    let notice = $state<string | null>(null);
    let pendingDelete = $state<string | null>(null);
    let testing = $state<string | null>(null);
    let expanded = $state<string | null>(null);
    let timer: ReturnType<typeof setTimeout> | null = null;

    let targetDialog = $state<ExportTargetDialog | null>(null);
    let exportDialog = $state<ExportDialog | null>(null);
    let restoreDialog = $state<RestoreDialog | null>(null);

    const hasRunning = $derived(exports.some((record) => record.status === "running"));

    function message(e: unknown, fallback: string): string {
        return e instanceof Error ? e.message : fallback;
    }

    async function loadTargets() {
        try {
            targets = await listExportTargets();
            error = null;
        } catch (e) {
            error = message(e, "Failed to load export targets");
        }
    }

    async function loadExports() {
        try {
            exports = await listExports();
            error = null;
        } catch (e) {
            error = message(e, "Failed to load exports");
        }
    }

    function schedule() {
        if (timer) clearTimeout(timer);
        if (!hasRunning) return;
        timer = setTimeout(async () => {
            await loadExports();
            schedule();
        }, POLL_INTERVAL_MS);
    }

    async function reloadExports() {
        await loadExports();
        schedule();
    }

    async function removeTarget(name: string) {
        try {
            await deleteExportTarget(name);
            pendingDelete = null;
            await loadTargets();
        } catch (e) {
            pendingDelete = null;
            error = message(e, "Failed to remove export target");
        }
    }

    async function test(name: string) {
        testing = name;
        notice = null;
        try {
            await testExportTarget(name);
            error = null;
            notice = `Target ${name} works`;
        } catch (e) {
            error = message(e, "Target test failed");
        } finally {
            testing = null;
        }
    }

    function formatDate(value: string | null): string {
        return value ? new Date(value).toLocaleString() : "-";
    }

    function totalBytes(record: ExportRecord): number {
        return record.manifest?.artifacts.reduce((sum, artifact) => sum + artifact.bytes, 0) ?? 0;
    }

    onMount(async () => {
        await Promise.all([loadTargets(), loadExports()]);
        loading = false;
        schedule();
    });

    onDestroy(() => {
        if (timer) clearTimeout(timer);
    });

    $effect(() => {
        if (hasRunning) schedule();
    });
</script>

<ExportTargetDialog bind:this={targetDialog} onSaved={loadTargets} />
<ExportDialog bind:this={exportDialog} {targets} onCreated={reloadExports} />
<RestoreDialog bind:this={restoreDialog} {targets} />

<SettingsPage title="Backups" description="Export the platform configuration and tenant data to a local path or S3 compatible storage, and restore from it.">
    {#snippet actions()}
        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => targetDialog?.open()}>
            <Plus class="cx-i" aria-hidden="true" />
            Add target
        </button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => exportDialog?.open()}>
            <Plus class="cx-i" aria-hidden="true" />
            New export
        </button>
    {/snippet}

    {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
    {#if notice}<p class="cx-hint" style="color: var(--success)">{notice}</p>{/if}

    {#if loading}
        <div class="db-skeleton db-skeleton--row" style="height: 96px" aria-hidden="true"></div>
    {:else}
        <section class="cx-panel st-panel" aria-labelledby="bk-targets">
            <div>
                <h3 id="bk-targets">Targets</h3>
                <p>Exports are written by the API gateway to a target. Encrypted targets protect exports before they leave the gateway.</p>
            </div>
            {#if targets.length === 0}
                <p class="st-note">No target yet. Add a local path or an S3 compatible bucket to start exporting.</p>
            {:else}
                <div>
                    {#each targets as target (target.name)}
                        <div class="st-role">
                            <div style="min-width: 0">
                                <h3>
                                    {target.name}
                                    <span class="cx-badge">{target.kind === "s3" ? "S3" : "Path"}</span>
                                    {#if target.default}<span class="cx-badge cx-badge--info">Default</span>{/if}
                                    {#if target.encrypted}<span class="cx-badge cx-badge--success">Encrypted</span>{/if}
                                </h3>
                                <p class="cx-mono" style="overflow-wrap: anywhere; font-size: 12px">{target.location}</p>
                            </div>
                            <div class="db-actions" style="flex-shrink: 0">
                                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={testing === target.name} aria-busy={testing === target.name} onclick={() => test(target.name)}>
                                    {testing === target.name ? "Testing" : "Test"}
                                </button>
                                {#if pendingDelete === target.name}
                                    <button class="cx-btn cx-btn--primary cx-btn--sm nd-danger" type="button" onclick={() => removeTarget(target.name)}>Confirm</button>
                                    <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (pendingDelete = null)}>Cancel</button>
                                {:else}
                                    <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (pendingDelete = target.name)}>Remove</button>
                                {/if}
                            </div>
                        </div>
                    {/each}
                </div>
            {/if}
        </section>

        <section class="cx-panel st-panel" aria-labelledby="bk-exports">
            <div>
                <h3 id="bk-exports">Exports</h3>
                <p>Removing a target does not delete exports that were already written to it.</p>
            </div>
            {#if exports.length === 0}
                <p class="st-note">No exports yet.</p>
            {:else}
                <div>
                    {#each exports as record (record.id)}
                        <div class="st-role" style="align-items: flex-start">
                            <div style="min-width: 0; flex: 1">
                                <h3>
                                    {scopeLabel(record.scope_kind)}
                                    <span class="cx-badge {statusBadge(record.status)}">{record.status}</span>
                                    {#if record.manifest?.encryption}<span class="cx-badge cx-badge--success">Encrypted</span>{/if}
                                </h3>
                                <p>
                                    {formatDate(record.created_at)}
                                    {#if record.manifest}&middot; {pluralize(record.manifest.artifacts.length, "artifact")} &middot; {formatBytes(totalBytes(record))}{/if}
                                </p>
                                <p class="cx-mono" style="overflow-wrap: anywhere; font-size: 12px">{record.id}</p>
                                {#if record.error}<p class="cx-hint cx-hint--error">{record.error}</p>{/if}
                                {#if expanded === record.id && record.manifest}
                                    <div class="st-note" style="display: grid; gap: 4px; margin-top: 8px">
                                        <span>CSFX {record.manifest.csfx_version} &middot; {record.manifest.consistency} consistency</span>
                                        {#each record.manifest.artifacts as artifact (artifact.name)}
                                            <span class="cx-mono" style="overflow-wrap: anywhere">{artifact.name} &middot; {formatBytes(artifact.bytes)}</span>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                            <div class="db-actions" style="flex-shrink: 0">
                                {#if record.manifest}
                                    <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (expanded = expanded === record.id ? null : record.id)}>
                                        {expanded === record.id ? "Hide" : "Details"}
                                    </button>
                                {/if}
                                {#if record.status === "completed" && RESTORABLE_SCOPES.includes(record.scope_kind)}
                                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => restoreDialog?.open(record)}>Restore</button>
                                {/if}
                            </div>
                        </div>
                    {/each}
                </div>
            {/if}
        </section>
    {/if}
</SettingsPage>
