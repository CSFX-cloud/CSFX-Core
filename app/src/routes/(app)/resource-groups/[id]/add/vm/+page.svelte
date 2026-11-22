<script lang="ts">
    import { goto } from "$app/navigation";
    import AddPage from "$lib/components/resource-group/add-page.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import WizardSteps from "$lib/components/shell/wizard-steps.svelte";
    import VmStepCompute from "$lib/components/vm/vm-step-compute.svelte";
    import { VmDraft } from "$lib/components/vm/vm-draft.svelte";
    import VmStepImage from "$lib/components/vm/vm-step-image.svelte";
    import VmStepNetwork from "$lib/components/vm/vm-step-network.svelte";
    import VmStepReview from "$lib/components/vm/vm-step-review.svelte";
    import VmStepStorage from "$lib/components/vm/vm-step-storage.svelte";

    const STEPS = ["Image", "Compute", "Storage", "Network", "Review"];
    const LAST_STEP = STEPS.length - 1;

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;
    const draft = new VmDraft();
    const cidr = $derived(rg.group?.internal_cidr ?? "");
    const usedIps = $derived(rg.workloads.flatMap((workload) => (workload.guest_ip ? [workload.guest_ip] : [])));

    let stepIndex = $state(0);
    let error = $state<string | null>(null);

    const stepReady = $derived(
        [draft.nameReady && draft.imageReady, draft.computeReady, draft.storageReady, draft.networkError(cidr, usedIps) === null, true][stepIndex]
    );

    function back() {
        error = null;
        stepIndex = Math.max(stepIndex - 1, 0);
    }

    async function deploy() {
        error = null;
        try {
            await draft.deploy(rg.rgId);
            await rg.load();
            goto(base);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to deploy vm";
        }
    }

    function advance() {
        if (!stepReady || draft.deploying) return;
        if (stepIndex < LAST_STEP) stepIndex += 1;
        else deploy();
    }
</script>

<AddPage type="vm" title="Deploy a virtual machine">
    <form
        style="display: grid; gap: 24px"
        onsubmit={(event) => {
            event.preventDefault();
            advance();
        }}
    >
        <WizardSteps steps={STEPS} current={stepIndex} label="Deploy a virtual machine" />

        <div class="md__step">
            {#if stepIndex === 0}
                <VmStepImage {draft} rgId={rg.rgId} onerror={(message) => (error = message)} />
            {:else if stepIndex === 1}
                <VmStepCompute {draft} />
            {:else if stepIndex === 2}
                <VmStepStorage {draft} volumes={rg.volumes} />
            {:else if stepIndex === 3}
                <VmStepNetwork {draft} groupName={rg.group?.name ?? ""} {cidr} {usedIps} />
            {:else}
                <VmStepReview {draft} volumes={rg.volumes} {cidr} />
            {/if}
        </div>

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <div class="db-actions">
            {#if stepIndex === 0}
                <a class="cx-btn cx-btn--ghost cx-btn--sm" href="{base}/add">Cancel</a>
            {:else}
                <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={back} disabled={draft.deploying}>Back</button>
            {/if}
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={!stepReady || draft.deploying} aria-busy={draft.deploying}>
                {stepIndex < LAST_STEP ? "Continue" : draft.deploying ? "Deploying" : "Deploy"}
            </button>
        </div>
    </form>
</AddPage>
