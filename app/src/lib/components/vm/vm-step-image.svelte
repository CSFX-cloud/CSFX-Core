<script lang="ts">
    import { Upload } from "@lucide/svelte";
    import { onMount } from "svelte";
    import { Select } from "$lib/components/ui/select";
    import { isoFileName, type VmDraft } from "./vm-draft.svelte";

    let { draft, rgId, onerror }: { draft: VmDraft; rgId: string; onerror: (message: string) => void } = $props();

    onMount(() => {
        draft.loadExistingIsos(rgId).catch((e) => onerror(e instanceof Error ? e.message : "Failed to load existing isos"));
    });

    function chooseFile(event: Event) {
        draft.isoFile = (event.target as HTMLInputElement).files?.[0] ?? null;
    }
</script>

<div class="md__fields md__fields--two">
    <div class="cx-field md__span">
        <label class="cx-label" for="vm-name">Name</label>
        <input id="vm-name" class="cx-input" placeholder="my-vm" bind:value={draft.name} />
    </div>
    <div class="cx-field md__span">
        <div class="md__foot" style="justify-content: space-between; align-items: center">
            <span class="cx-label">ISO image</span>
            {#if draft.existingIsos.length > 0}
                <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="ISO source">
                    <button class="cx-tab" role="tab" type="button" aria-selected={draft.isoMode === "existing"} onclick={() => (draft.isoMode = "existing")}>Existing</button>
                    <button class="cx-tab" role="tab" type="button" aria-selected={draft.isoMode === "upload"} onclick={() => (draft.isoMode = "upload")}>Upload new</button>
                </div>
            {/if}
        </div>
        {#if draft.existingLoading}
            <p class="cx-hint">Loading isos</p>
        {:else if draft.isoMode === "existing"}
            <Select
                id="vm-iso-existing"
                bind:value={draft.selectedIsoKey}
                options={draft.existingIsos.map((object) => ({ value: object.key, label: isoFileName(object.key) }))}
            />
        {:else}
            <div class="md__foot" style="justify-content: flex-start; align-items: center">
                <label class="cx-btn cx-btn--secondary cx-btn--sm" style="cursor: pointer">
                    <Upload class="cx-i" aria-hidden="true" />
                    Choose ISO
                    <input id="vm-iso" type="file" accept=".iso" class="db-sr-only" onchange={chooseFile} />
                </label>
                <span class="cx-hint">{draft.isoFile?.name ?? "No file selected"}</span>
            </div>
        {/if}
        <p class="cx-hint">The ISO boots on first start. Its architecture has to match the one you pick in the next step.</p>
    </div>
</div>
