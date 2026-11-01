<script lang="ts">
    import { ChevronLeft } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import { createSite, detectSite, type Detection, type SiteMode } from "$lib/api/sites";
    import { listSecrets } from "$lib/api/vaults";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";

    const DEFAULT_BRANCH = "main";
    const DEFAULT_PORT = 3000;

    const rg = useResourceGroupState();
    const base = `/resource-groups/${rg.rgId}`;

    let repoUrl = $state("");
    let branch = $state(DEFAULT_BRANCH);
    let rootDir = $state("");
    let vaultId = $state("");
    let tokenSecret = $state("");
    let tokenNames = $state<string[]>([]);
    let name = $state("");
    let gatewayId = $state("");
    let mode = $state<SiteMode>("static");
    let installCommand = $state("");
    let buildCommand = $state("");
    let outputDir = $state("");
    let startCommand = $state("");
    let containerPort = $state(DEFAULT_PORT);
    let previews = $state(true);
    let detection = $state<Detection | null>(null);
    let inspected = $state(false);
    let inspecting = $state(false);
    let creating = $state(false);
    let error = $state<string | null>(null);

    const usableVaults = $derived(rg.vaults.filter((vault) => !vault.sealed));
    const canInspect = $derived(repoUrl.trim().startsWith("http") && !inspecting);
    const canCreate = $derived(name.trim() !== "" && gatewayId !== "" && inspected && !creating);

    $effect(() => {
        breadcrumb.set(rg.group ? `${rg.group.name} / Add web app` : null);
        return () => breadcrumb.set(null);
    });

    $effect(() => {
        if (!gatewayId && rg.gateways.length > 0) gatewayId = rg.gateways[0].id;
    });

    async function loadTokenNames() {
        tokenSecret = "";
        tokenNames = [];
        if (!auth.token || !vaultId) return;
        const secrets = await listSecrets(auth.token, rg.rgId, vaultId).catch(() => []);
        tokenNames = secrets.filter((secret) => !secret.deleted).map((secret) => secret.name);
    }

    function applyDetection(found: Detection | null) {
        detection = found;
        if (!found) return;
        mode = found.mode;
        installCommand = found.install_command ?? "";
        buildCommand = found.build_command ?? "";
        outputDir = found.output_dir ?? "";
        startCommand = found.start_command ?? "";
        containerPort = found.port ?? DEFAULT_PORT;
        if (!name) name = repoUrl.split("/").pop()?.replace(/\.git$/, "").toLowerCase().replace(/[^a-z0-9-]/g, "-") ?? "";
    }

    async function inspect() {
        if (!auth.token) return;
        inspecting = true;
        error = null;
        try {
            const result = await detectSite(auth.token, rg.rgId, {
                repo_url: repoUrl.trim(),
                branch: branch.trim() || DEFAULT_BRANCH,
                root_dir: rootDir.trim(),
                vault_id: vaultId || undefined,
                git_token_secret: tokenSecret || undefined,
            });
            applyDetection(result.detection);
            inspected = true;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to inspect the repository";
        } finally {
            inspecting = false;
        }
    }

    function optional(value: string): string | undefined {
        return value.trim() === "" ? undefined : value.trim();
    }

    async function submit() {
        if (!auth.token || !canCreate) return;
        creating = true;
        error = null;
        try {
            const site = await createSite(auth.token, rg.rgId, {
                name: name.trim(),
                repo_url: repoUrl.trim(),
                branch: branch.trim() || DEFAULT_BRANCH,
                root_dir: rootDir.trim(),
                vault_id: vaultId || undefined,
                git_token_secret: tokenSecret || undefined,
                gateway_id: gatewayId,
                preview_enabled: mode === "static" && previews,
                mode,
                install_command: optional(installCommand),
                build_command: optional(buildCommand),
                output_dir: mode === "static" ? optional(outputDir) : undefined,
                start_command: mode === "container" ? optional(startCommand) : undefined,
                container_port: mode === "container" ? containerPort : undefined,
            });
            await rg.load();
            await goto(`${base}/site/${site.id}`);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create web app";
            creating = false;
        }
    }
</script>

<div class="db-content">
    <section class="cx-stage" aria-label="Add web app">
        <div class="cx-stage__canvas"></div>
        <div class="ga-head">
            <div style="display: grid; gap: 10px">
                <a class="ga-back" href="{base}/add">
                    <ChevronLeft class="cx-i" aria-hidden="true" />
                    Back to resources
                </a>
                <div class="rs-name">
                    <ServiceIcon name="web-apps" size="xl" />
                    <h1 class="db-title">Deploy a web app.<br /><span style="color: var(--ink-muted)">Pull a repository, build it, serve it.</span></h1>
                </div>
            </div>
        </div>
    </section>

    <form
        style="display: grid; gap: 24px"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <section class="cx-panel" aria-labelledby="wa-source">
            <div class="cx-panel__head">
                <div>
                    <h2 class="cx-panel__title" id="wa-source">Repository</h2>
                    <p class="cx-panel__sub">Public repositories work as they are. For private ones pick a key vault secret that holds an access token.</p>
                </div>
            </div>
            <div class="md__fields" style="padding: 0 24px 24px">
                <div class="cx-field">
                    <label class="cx-label" for="wa-repo">Repository URL</label>
                    <input class="cx-input" id="wa-repo" placeholder="https://github.com/org/repo" autocomplete="off" bind:value={repoUrl} oninput={() => (inspected = false)} />
                </div>
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                    <div class="cx-field">
                        <label class="cx-label" for="wa-branch">Production branch</label>
                        <input class="cx-input" id="wa-branch" autocomplete="off" bind:value={branch} oninput={() => (inspected = false)} />
                    </div>
                    <div class="cx-field">
                        <label class="cx-label" for="wa-root">Root directory</label>
                        <input class="cx-input" id="wa-root" placeholder="repository root" autocomplete="off" bind:value={rootDir} oninput={() => (inspected = false)} />
                    </div>
                </div>
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                    <div class="cx-field">
                        <label class="cx-label" for="wa-vault">Key vault</label>
                        <select class="cx-input" id="wa-vault" bind:value={vaultId} onchange={loadTokenNames}>
                            <option value="">None, public repository</option>
                            {#each usableVaults as vault (vault.id)}<option value={vault.id}>{vault.name}</option>{/each}
                        </select>
                    </div>
                    <div class="cx-field">
                        <label class="cx-label" for="wa-token">Access token secret</label>
                        <select class="cx-input" id="wa-token" bind:value={tokenSecret} disabled={tokenNames.length === 0} onchange={() => (inspected = false)}>
                            <option value="">Select</option>
                            {#each tokenNames as secretName (secretName)}<option value={secretName}>{secretName}</option>{/each}
                        </select>
                    </div>
                </div>
                <div>
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={!canInspect} aria-busy={inspecting} onclick={inspect}>
                        {inspecting ? "Inspecting" : "Inspect repository"}
                    </button>
                </div>
                {#if inspected}
                    <p class="cx-hint">
                        {#if detection}
                            Detected <b>{detection.framework ?? "project"}</b>, suggested as {detection.mode === "static" ? "a static site" : "a container app"}. You can change everything below.
                        {:else}
                            Nothing recognised. Choose how to run it below.
                        {/if}
                    </p>
                {/if}
            </div>
        </section>

        {#if inspected}
            <section class="cx-panel" aria-labelledby="wa-build">
                <div class="cx-panel__head">
                    <div>
                        <h2 class="cx-panel__title" id="wa-build">Build and run</h2>
                        <p class="cx-panel__sub">Builds run in an isolated Firecracker microVM.</p>
                    </div>
                    <div class="cx-tabs cx-tabs--contained cx-tabs--sm" role="tablist" aria-label="Deployment type">
                        <button class="cx-tab" role="tab" type="button" aria-selected={mode === "static"} onclick={() => (mode = "static")}>Static site</button>
                        <button class="cx-tab" role="tab" type="button" aria-selected={mode === "container"} onclick={() => (mode = "container")}>Container app</button>
                    </div>
                </div>
                <div class="md__fields" style="padding: 0 24px 24px">
                    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                        <div class="cx-field">
                            <label class="cx-label" for="wa-name">Name</label>
                            <input class="cx-input" id="wa-name" placeholder="shop-web" autocomplete="off" bind:value={name} />
                            <p class="cx-hint">Lowercase letters, digits and hyphens.</p>
                        </div>
                        <div class="cx-field">
                            <label class="cx-label" for="wa-gateway">Gateway</label>
                            <select class="cx-input" id="wa-gateway" bind:value={gatewayId} disabled={rg.gateways.length === 0}>
                                {#each rg.gateways as gateway (gateway.id)}<option value={gateway.id}>{gateway.name}</option>{/each}
                            </select>
                            {#if rg.gateways.length === 0}<p class="cx-hint cx-hint--error">Create an API gateway in this group first.</p>{/if}
                        </div>
                    </div>
                    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                        <div class="cx-field">
                            <label class="cx-label" for="wa-install">Install command</label>
                            <input class="cx-input" id="wa-install" autocomplete="off" bind:value={installCommand} />
                        </div>
                        <div class="cx-field">
                            <label class="cx-label" for="wa-build-cmd">Build command</label>
                            <input class="cx-input" id="wa-build-cmd" autocomplete="off" bind:value={buildCommand} />
                        </div>
                    </div>
                    {#if mode === "static"}
                        <div class="cx-field">
                            <label class="cx-label" for="wa-output">Output directory</label>
                            <input class="cx-input" id="wa-output" placeholder="dist" autocomplete="off" bind:value={outputDir} />
                        </div>
                        <label class="cx-check"><input type="checkbox" bind:checked={previews} /> Preview every pull request</label>
                    {:else}
                        <div style="display: grid; grid-template-columns: 2fr 1fr; gap: 12px">
                            <div class="cx-field">
                                <label class="cx-label" for="wa-start">Start command</label>
                                <input class="cx-input" id="wa-start" placeholder="node build" autocomplete="off" bind:value={startCommand} />
                                <p class="cx-hint">Ignored when the repository has a Dockerfile.</p>
                            </div>
                            <div class="cx-field">
                                <label class="cx-label" for="wa-port">Port</label>
                                <input class="cx-input" id="wa-port" type="number" min="1" max="65535" bind:value={containerPort} />
                            </div>
                        </div>
                    {/if}
                </div>
            </section>
        {/if}

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <div class="db-actions">
            <a class="cx-btn cx-btn--ghost cx-btn--sm" href="{base}/add">Cancel</a>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={!canCreate} aria-busy={creating}>{creating ? "Deploying" : "Deploy"}</button>
        </div>
    </form>
</div>
