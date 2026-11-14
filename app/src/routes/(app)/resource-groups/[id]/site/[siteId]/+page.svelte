<script lang="ts">
    import { toneBadgeClass } from "$lib/utils/tone";
    import { onMount, untrack } from "svelte";
    import { ExternalLink, Rocket, Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/stores";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteSite, getSite, redeploySite, type SiteDetail } from "$lib/api/sites";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import BuildLog from "$lib/components/site/build-log.svelte";
    import BuildPanel from "$lib/components/site/build-panel.svelte";
    import DeploymentsTab from "$lib/components/site/deployments-tab.svelte";
    import DomainsPanel from "$lib/components/site/domains-panel.svelte";
    import EnvPanel from "$lib/components/site/env-panel.svelte";
    import OverviewTab from "$lib/components/site/overview-tab.svelte";
    import WebhookPanel from "$lib/components/site/webhook-panel.svelte";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";
    import { buildDuration, deploymentStatus, repoLabel } from "$lib/utils/site-status";

    type Tab = "overview" | "build" | "deployments" | "settings";

    const TABS: { id: Tab; label: string }[] = [
        { id: "overview", label: "Overview" },
        { id: "build", label: "Build log" },
        { id: "deployments", label: "Deployments" },
        { id: "settings", label: "Settings" },
    ];
    const POLL_INTERVAL_MS = 4000;

    const rg = useResourceGroupState();
    const siteId = $page.params.siteId ?? "";
    const base = `/resource-groups/${rg.rgId}`;

    let site = $state<SiteDetail | null>(null);
    let tab = $state<Tab>("overview");
    let loading = $state(true);
    let busy = $state(false);
    let error = $state<string | null>(null);

    const latest = $derived(site?.deployments[0]);
    const status = $derived(deploymentStatus(latest?.status));
    const building = $derived(latest?.status === "queued" || latest?.status === "building");
    const liveDomain = $derived(site?.gateway_id ? site.domains.find((domain) => domain.kind === "production" || domain.kind === "auto") : undefined);
    const lastBuild = $derived(site?.deployments.find((deployment) => deployment.duration_ms !== null));

    async function load() {
        if (!auth.token) return;
        try {
            site = await getSite(auth.token, rg.rgId, siteId);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load web app";
        } finally {
            loading = false;
        }
    }

    async function redeploy() {
        if (!auth.token) return;
        busy = true;
        try {
            await redeploySite(auth.token, rg.rgId, siteId);
            await load();
            tab = "overview";
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to start deployment";
        } finally {
            busy = false;
        }
    }

    async function remove() {
        if (!auth.token || !site || !confirm(`Delete ${site.name}? Its deployments and domains are removed.`)) return;
        busy = true;
        try {
            await deleteSite(auth.token, rg.rgId, siteId);
            await rg.load();
            await goto(base);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete web app";
            busy = false;
        }
    }

    onMount(() => {
        const interval = setInterval(() => {
            if (building) load();
        }, POLL_INTERVAL_MS);
        return () => clearInterval(interval);
    });

    $effect(() => {
        if (auth.token) untrack(load);
    });

    $effect(() => {
        breadcrumb.set(site?.name ?? siteId.slice(0, 8));
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    {#if loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !site}
        <p class="cx-hint cx-hint--error">{error ?? "Web app not found"}</p>
    {:else}
        <div class="db-head">
            <div class="rs-name">
                <ServiceIcon name="web-apps" size="xl" />
                <div>
                    <h1 class="db-title">{site.name}</h1>
                    <p class="db-sub">
                        <span class={toneBadgeClass(status.tone)}>{status.label}</span>
                        <span class="cx-soft">{repoLabel(site.repo_url)} &middot; {site.production_branch}</span>
                    </p>
                </div>
            </div>
            <div class="db-actions">
                {#if liveDomain}
                    <a class="cx-btn cx-btn--secondary cx-btn--sm" href="https://{liveDomain.hostname}" target="_blank" rel="noreferrer">
                        <ExternalLink class="cx-i" aria-hidden="true" />
                        Visit
                    </a>
                {/if}
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={busy || building} onclick={redeploy}>
                    <Rocket class="cx-i" aria-hidden="true" />
                    Redeploy
                </button>
                <button class="cx-icon-btn nd-danger" type="button" disabled={busy} aria-label="Delete web app" title="Delete web app" onclick={remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            </div>
        </div>

        <section class="db-stats" aria-label="Web app summary">
            <div class="cx-stat">
                <p class="cx-stat__label">Type</p>
                <p class="db-stat-value">{site.mode === "static" ? "Static" : "Container"}</p>
                <div class="cx-stat__foot">{site.framework ?? "custom"}</div>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Last build</p>
                <p class="db-stat-value">{buildDuration(lastBuild?.duration_ms ?? null)}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Deployments</p>
                <p class="db-stat-value">{site.deployments.length}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Domains</p>
                <p class="db-stat-value">{site.domains.length}</p>
            </div>
        </section>

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <WorkloadTabs tabs={TABS} active={tab} label="Web app sections" onselect={(id) => (tab = id)} />

        {#if tab === "overview"}
            <OverviewTab rgId={rg.rgId} {site} />
        {:else if tab === "build"}
            {#if latest}
                {#key latest.id}
                    <BuildLog rgId={rg.rgId} siteId={site.id} deploymentId={latest.id} onSettled={load} />
                {/key}
            {:else}
                <p class="cx-hint">Nothing has been built yet.</p>
            {/if}
        {:else if tab === "deployments"}
            <DeploymentsTab rgId={rg.rgId} siteId={site.id} deployments={site.deployments} onChanged={load} onError={(message) => (error = message)} />
        {:else}
            <BuildPanel rgId={rg.rgId} {site} onChanged={load} onError={(message) => (error = message)} />
            <EnvPanel rgId={rg.rgId} {site} onChanged={load} onError={(message) => (error = message)} />
            <DomainsPanel rgId={rg.rgId} {site} onChanged={load} onError={(message) => (error = message)} />
            <WebhookPanel {site} />
        {/if}
    {/if}
</div>
