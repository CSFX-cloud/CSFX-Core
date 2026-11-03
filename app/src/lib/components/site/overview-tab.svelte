<script lang="ts">
    import { ExternalLink } from "@lucide/svelte";
    import type { DeploymentSummary, SiteDetail } from "$lib/api/sites";
    import { relativeTime } from "$lib/utils/dashboard";
    import { badgeClass, buildDuration, deploymentStatus, shortSha } from "$lib/utils/site-status";
    import BuildLog from "./build-log.svelte";
    import UsageStats from "./usage-stats.svelte";

    const PREVIEW_LIMIT = 8;

    type Props = { rgId: string; site: SiteDetail; onChanged: () => Promise<void> };

    let { rgId, site, onChanged }: Props = $props();

    const production = $derived(site.deployments.find((deployment) => deployment.current) ?? null);
    const latest = $derived(site.deployments[0] ?? null);
    const previews = $derived(site.deployments.filter((deployment) => deployment.environment === "preview").slice(0, PREVIEW_LIMIT));
    const productionHost = $derived(site.domains.find((domain) => domain.kind !== "branch")?.hostname ?? null);
    const previewHosts = $derived(site.domains.filter((domain) => domain.kind === "branch"));

    function previewHost(deployment: DeploymentSummary): string | null {
        return previewHosts.find((domain) => domain.branch === deployment.branch)?.hostname ?? null;
    }
</script>

<UsageStats {rgId} siteId={site.id} mode={site.mode} />

<section class="cx-panel" aria-labelledby="ov-prod">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="ov-prod">Production</h2>
            <p class="cx-panel__sub">What visitors see right now</p>
        </div>
    </div>
    {#if production}
        {@const status = deploymentStatus(production.status)}
        <dl class="ov-grid">
            <div><dt>Status</dt><dd><span class={badgeClass(status.tone)}>{status.label}</span></dd></div>
            <div><dt>Deployed</dt><dd>{relativeTime(production.created_at, Date.now())}</dd></div>
            <div><dt>Commit</dt><dd>{production.commit_message || "No message"}<br /><span class="cx-soft">{shortSha(production.commit_sha)} &middot; {production.branch}</span></dd></div>
            <div><dt>Build time</dt><dd>{buildDuration(production.duration_ms)}</dd></div>
            <div>
                <dt>Domains</dt>
                <dd>
                    {#each site.domains.filter((domain) => domain.kind !== "branch") as domain (domain.id)}
                        <a class="cx-link" href="https://{domain.hostname}" target="_blank" rel="noreferrer">{domain.hostname}<ExternalLink class="cx-i" aria-hidden="true" /></a><br />
                    {:else}
                        <span class="cx-soft">No domain yet</span>
                    {/each}
                </dd>
            </div>
        </dl>
        {#if productionHost && production.status === "ready"}
            <iframe class="ov-frame" title="Live preview of {site.name}" src="https://{productionHost}" sandbox="allow-scripts allow-same-origin allow-forms" loading="lazy"></iframe>
        {/if}
    {:else}
        <p class="cx-hint" style="padding: 0 24px 24px">
            {latest ? "The first deployment has not finished. Production appears once it is ready." : "Nothing is deployed yet."}
        </p>
    {/if}
</section>

{#if previews.length > 0}
    <section class="cx-panel" aria-labelledby="ov-prev">
        <div class="cx-panel__head">
            <div>
                <h2 class="cx-panel__title" id="ov-prev">Preview deployments</h2>
                <p class="cx-panel__sub">One live URL per open pull request</p>
            </div>
        </div>
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Branch</th><th>Commit</th><th>URL</th><th>Status</th><th>Age</th></tr></thead>
                <tbody>
                    {#each previews as deployment (deployment.id)}
                        {@const status = deploymentStatus(deployment.status)}
                        {@const host = previewHost(deployment)}
                        <tr>
                            <td>{deployment.branch}</td>
                            <td class="cx-soft">{shortSha(deployment.commit_sha)}</td>
                            <td>
                                {#if host && deployment.status === "ready"}
                                    <a class="cx-link" href="https://{host}" target="_blank" rel="noreferrer">{host}</a>
                                {:else}
                                    <span class="cx-soft">&mdash;</span>
                                {/if}
                            </td>
                            <td><span class={badgeClass(status.tone)}>{status.label}</span></td>
                            <td class="cx-soft">{relativeTime(deployment.created_at, Date.now())}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    </section>
{/if}

{#if latest}
    {#key latest.id}
        <BuildLog {rgId} siteId={site.id} deploymentId={latest.id} onSettled={onChanged} />
    {/key}
{/if}

<style>
    .ov-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: var(--space-4);
        margin: 0;
        padding: 0 var(--space-5) var(--space-5);
    }
    .ov-frame {
        display: block;
        width: calc(100% - 2 * var(--space-5));
        height: 420px;
        margin: 0 var(--space-5) var(--space-5);
        border: 1px solid var(--border, currentColor);
        border-radius: 8px;
        background: #fff;
    }
    .ov-grid dt {
        font-size: 12px;
        color: var(--ink-muted);
        margin-bottom: 4px;
    }
    .ov-grid dd {
        margin: 0;
        font-size: 14px;
        color: var(--ink);
    }
</style>
