<script lang="ts">
    import { untrack } from "svelte";
    import { Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/stores";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteRegistry, getRegistry, type RegistryDetail, type Repository } from "$lib/api/registries";
    import AccessTab from "$lib/components/registry/access-tab.svelte";
    import ImagesTab from "$lib/components/registry/images-tab.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";

    type Tab = "images" | "access";

    const TABS: { id: Tab; label: string }[] = [
        { id: "images", label: "Images" },
        { id: "access", label: "Access" },
    ];
    const REGISTRY_PORT = 5060;

    const rg = useResourceGroupState();
    const registryId = $page.params.registryId ?? "";
    const base = `/resource-groups/${rg.rgId}`;
    const host = `${$page.url.hostname}:${REGISTRY_PORT}`;

    let registry = $state<RegistryDetail | null>(null);
    let repositories = $state<Repository[]>([]);
    let tab = $state<Tab>("images");
    let loading = $state(true);
    let busy = $state(false);
    let error = $state<string | null>(null);

    const imageCount = $derived(repositories.reduce((count, repository) => count + repository.tags.length, 0));

    async function load() {
        try {
            registry = await getRegistry(rg.rgId, registryId);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load registry";
        } finally {
            loading = false;
        }
    }

    async function remove() {
        if (!registry || !confirm(`Delete ${registry.name} and all of its images? This cannot be undone.`)) return;
        busy = true;
        try {
            await deleteRegistry(rg.rgId, registryId);
            await rg.load();
            await goto(base);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete registry";
            busy = false;
        }
    }

    $effect(() => {
        if (auth.token) untrack(load);
    });

    $effect(() => {
        breadcrumb.set(registry?.name ?? registryId.slice(0, 8));
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    {#if loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !registry}
        <p class="cx-hint cx-hint--error">{error ?? "Registry not found"}</p>
    {:else}
        <div class="db-head">
            <div class="rs-name">
                <ServiceIcon name="container-registry" size="xl" />
                <div>
                    <h1 class="db-title">{registry.name}</h1>
                    <p class="db-sub">
                        <code>{host}/{registry.prefix}</code>
                        <a class="cx-link" href={base}>{rg.group?.name ?? "Resource group"}</a>
                    </p>
                </div>
            </div>
            <div class="db-actions">
                <button class="cx-icon-btn nd-danger" type="button" disabled={busy} aria-label="Delete registry" title="Delete registry" onclick={remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            </div>
        </div>

        <section class="db-stats" aria-label="Registry summary">
            <div class="cx-stat">
                <p class="cx-stat__label">Repositories</p>
                <p class="db-stat-value">{repositories.length}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Images</p>
                <p class="db-stat-value">{imageCount}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Access tokens</p>
                <p class="db-stat-value">{registry.tokens.length}</p>
            </div>
        </section>

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <WorkloadTabs tabs={TABS} active={tab} label="Registry sections" onselect={(id) => (tab = id)} />

        {#if tab === "images"}
            <ImagesTab rgId={rg.rgId} {registry} {host} onLoaded={(loaded) => (repositories = loaded)} onError={(message) => (error = message)} />
        {:else}
            <AccessTab rgId={rg.rgId} {registry} {host} onChanged={load} onError={(message) => (error = message)} />
        {/if}
    {/if}
</div>
