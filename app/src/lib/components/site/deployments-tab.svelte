<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { promoteDeployment, type DeploymentSummary } from "$lib/api/sites";
    import { relativeTime } from "$lib/utils/dashboard";
    import { badgeClass, buildDuration, deploymentStatus, shortSha } from "$lib/utils/site-status";
    import BuildLog from "./build-log.svelte";

    type Props = {
        rgId: string;
        siteId: string;
        deployments: DeploymentSummary[];
        onChanged: () => Promise<void>;
        onError: (message: string) => void;
    };

    let { rgId, siteId, deployments, onChanged, onError }: Props = $props();

    let selected = $state<string | null>(null);
    let busy = $state<string | null>(null);

    function actionLabel(deployment: DeploymentSummary): string | null {
        if (deployment.status !== "ready" || deployment.current || deployment.expired) return null;
        const current = deployments.find((candidate) => candidate.current);
        const older = current && new Date(deployment.created_at) < new Date(current.created_at);
        return older ? "Roll back to this" : "Promote";
    }

    async function promote(deployment: DeploymentSummary) {
        if (!auth.token) return;
        busy = deployment.id;
        try {
            await promoteDeployment(auth.token, rgId, siteId, deployment.id);
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to promote deployment");
        } finally {
            busy = null;
        }
    }
</script>

<section class="cx-panel" aria-labelledby="dp-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="dp-title">Deployments</h2>
            <p class="cx-panel__sub">Every push builds. Production only changes from the production branch, or when you promote a preview.</p>
        </div>
    </div>
    <div class="db-scroll">
        <table class="cx-table">
            <thead><tr><th>Status</th><th>Commit</th><th>Environment</th><th>Age</th><th>Build</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
            <tbody>
                {#each deployments as deployment (deployment.id)}
                    {@const status = deploymentStatus(deployment.status)}
                    {@const label = actionLabel(deployment)}
                    <tr>
                        <td>
                            <span class={badgeClass(status.tone)}>{status.label}</span>
                            {#if deployment.current}<span class="cx-badge">current</span>{/if}
                            {#if deployment.expired}<span class="cx-badge cx-badge--warning" title="The build output was removed to save space">expired</span>{/if}
                        </td>
                        <td>
                            <div class="rs-text">
                                <span>{deployment.commit_message || "No message"}</span>
                                <span class="cx-soft">{shortSha(deployment.commit_sha)} &middot; {deployment.branch}{deployment.pull_request ? ` &middot; PR ${deployment.pull_request}` : ""}</span>
                            </div>
                        </td>
                        <td class="cx-soft">{deployment.environment === "production" ? "Production" : "Preview"}</td>
                        <td class="cx-soft">{relativeTime(deployment.created_at, Date.now())}</td>
                        <td class="cx-num">{buildDuration(deployment.duration_ms)}</td>
                        <td>
                            <div class="db-actions" style="justify-content: flex-end">
                                <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (selected = deployment.id)}>Build log</button>
                                {#if label}
                                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={busy === deployment.id} onclick={() => promote(deployment)}>{label}</button>
                                {/if}
                            </div>
                        </td>
                    </tr>
                {:else}
                    <tr><td colspan="6" class="cx-soft">No deployments yet.</td></tr>
                {/each}
            </tbody>
        </table>
    </div>
</section>

{#if selected}
    {#key selected}
        <BuildLog {rgId} {siteId} deploymentId={selected} onSettled={onChanged} />
    {/key}
{/if}
