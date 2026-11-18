<script lang="ts">
    import { createExport, listScopeResources, type ExportTarget, type ScopeKind, type ScopeResource } from "$lib/api/exports";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";
    import { SCOPES } from "$lib/utils/exports";

    let { targets, onCreated }: { targets: ExportTarget[]; onCreated: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let kind = $state<ScopeKind>("config");
    let resources = $state<ScopeResource[]>([]);
    let resourceId = $state("");
    let targetName = $state("");
    let loadingResources = $state(false);
    let saving = $state(false);
    let error = $state<string | null>(null);

    const scope = $derived(SCOPES.find((item) => item.kind === kind) ?? SCOPES[0]);
    const ready = $derived(targets.length > 0 && (!scope.needsResource || resourceId !== ""));

    export function open() {
        kind = "config";
        resources = [];
        resourceId = "";
        targetName = targets.find((target) => target.default)?.name ?? targets[0]?.name ?? "";
        error = null;
        dialog?.open();
    }

    async function selectKind(next: ScopeKind) {
        kind = next;
        resourceId = "";
        resources = [];
        if (!SCOPES.find((item) => item.kind === next)?.needsResource) return;
        loadingResources = true;
        try {
            resources = await listScopeResources(next);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load resources";
        } finally {
            loadingResources = false;
        }
    }

    async function submit() {
        if (!ready) return;
        saving = true;
        error = null;
        try {
            await createExport({ kind, id: scope.needsResource ? resourceId : null }, targetName === "" ? null : targetName);
            dialog?.close();
            await onCreated();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to start export";
        } finally {
            saving = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="New export" subtitle="Choose what to export and where to write it" size="lg" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        {#if targets.length === 0}
            <p class="cx-hint cx-hint--error">Add an export target first.</p>
        {/if}
        <div class="cx-field">
            <label class="cx-label" for="export-scope">Scope</label>
            <select class="cx-input" id="export-scope" value={kind} onchange={(event) => selectKind(event.currentTarget.value as ScopeKind)}>
                {#each SCOPES as item (item.kind)}
                    <option value={item.kind}>{item.label}</option>
                {/each}
            </select>
            <p class="cx-hint">{scope.description}</p>
        </div>
        {#if scope.needsResource}
            <div class="cx-field">
                <label class="cx-label" for="export-resource">{scope.label}</label>
                <select class="cx-input" id="export-resource" bind:value={resourceId} disabled={loadingResources}>
                    <option value="" disabled>{loadingResources ? "Loading" : "Select"}</option>
                    {#each resources as resource (resource.id)}
                        <option value={resource.id}>{resource.name}</option>
                    {/each}
                </select>
            </div>
        {/if}
        <div class="cx-field">
            <label class="cx-label" for="export-target">Target</label>
            <select class="cx-input" id="export-target" bind:value={targetName}>
                {#each targets as target (target.name)}
                    <option value={target.name}>{target.name}{target.default ? " (default)" : ""}{target.encrypted ? ", encrypted" : ""}</option>
                {/each}
            </select>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving || !ready} aria-busy={saving}>
                {saving ? "Starting" : "Start export"}
            </button>
        </div>
    </form>
</ModalDialog>
