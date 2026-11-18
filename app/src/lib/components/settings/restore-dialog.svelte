<script lang="ts">
    import { listNodes, type Node } from "$lib/api/nodes";
    import { restoreExport, type ExportRecord, type ExportTarget } from "$lib/api/exports";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";
    import { scopeLabel } from "$lib/utils/exports";

    let { targets }: { targets: ExportTarget[] } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let record = $state<ExportRecord | null>(null);
    let targetName = $state("");
    let passphrase = $state("");
    let agentId = $state("");
    let nodes = $state<Node[]>([]);
    let restoring = $state(false);
    let error = $state<string | null>(null);
    let outcome = $state<string | null>(null);

    const encrypted = $derived(record?.manifest?.encryption != null);
    const needsAgent = $derived(record?.scope_kind === "workload" || record?.scope_kind === "resource_group");

    const consequence = $derived.by(() => {
        switch (record?.scope_kind) {
            case "config":
                return "Matching configuration rows are overwritten. Nothing is deleted.";
            case "volume":
                return "A new volume is created. The original is not touched.";
            case "bucket":
                return "A new bucket is created. The original is not touched.";
            case "workload":
                return "The boot disk is pushed to an agent. It is refused if the vm runs there or a disk already exists.";
            default:
                return "Each volume, bucket and boot disk is restored in turn. The first failure stops the restore.";
        }
    });

    export async function open(next: ExportRecord) {
        record = next;
        targetName = targets.find((target) => target.default)?.name ?? targets[0]?.name ?? "";
        passphrase = "";
        agentId = "";
        error = null;
        outcome = null;
        dialog?.open();
        if (nodes.length === 0 && (next.scope_kind === "workload" || next.scope_kind === "resource_group")) {
            nodes = await listNodes().catch(() => []);
        }
    }

    async function restore() {
        if (!record) return;
        restoring = true;
        error = null;
        try {
            const result = await restoreExport(record.id, {
                target: targetName === "" ? null : targetName,
                passphrase: passphrase === "" ? null : passphrase,
                agent_id: agentId === "" ? null : agentId,
            });
            outcome = Object.keys(result ?? {}).length === 0 ? "Restore completed" : JSON.stringify(result, null, 2);
        } catch (e) {
            error = e instanceof Error ? e.message : "Restore failed";
        } finally {
            restoring = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Restore export" subtitle={record ? `${scopeLabel(record.scope_kind)} ${record.id}` : undefined} size="lg">
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            restore();
        }}
    >
        <div class="st-note">{consequence}</div>
        <div class="cx-field">
            <label class="cx-label" for="restore-target">Read from target</label>
            <select class="cx-input" id="restore-target" bind:value={targetName}>
                {#each targets as target (target.name)}
                    <option value={target.name}>{target.name}{target.default ? " (default)" : ""}</option>
                {/each}
            </select>
        </div>
        {#if encrypted}
            <div class="cx-field">
                <label class="cx-label" for="restore-passphrase">Passphrase</label>
                <input class="cx-input" id="restore-passphrase" type="password" autocomplete="off" placeholder="Optional, the target's passphrase is used when empty" bind:value={passphrase} />
            </div>
        {/if}
        {#if needsAgent}
            <div class="cx-field">
                <label class="cx-label" for="restore-agent">Agent for vm boot disks</label>
                <select class="cx-input" id="restore-agent" bind:value={agentId}>
                    <option value="">The agent the workload is assigned to</option>
                    {#each nodes as node (node.id)}
                        <option value={node.id}>{node.name}</option>
                    {/each}
                </select>
            </div>
        {/if}
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        {#if outcome}<pre class="st-note cx-mono" style="overflow: auto; max-height: 240px">{outcome}</pre>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>{outcome ? "Close" : "Cancel"}</button>
            {#if !outcome}
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={restoring || targets.length === 0} aria-busy={restoring}>
                    {restoring ? "Restoring" : "Restore"}
                </button>
            {/if}
        </div>
    </form>
</ModalDialog>
