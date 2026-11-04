<script lang="ts">
    import { ChevronLeft } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import {
        createSite,
        detectPackageManager,
        detectSite,
        FRAMEWORK_PRESETS,
        PACKAGE_MANAGERS,
        withPackageManager,
        type Detection,
        type PackageManager,
        type SiteMode,
    } from "$lib/api/sites";
    import { listSecrets } from "$lib/api/vaults";
    import CreateGatewayDialog from "$lib/components/resource-group/create-gateway-dialog.svelte";
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
    let gatewayDialog = $state<CreateGatewayDialog | null>(null);
    let framework = $state("");
    let packageManager = $state<PackageManager>("npm");
    let inspectRun = 0;
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
    const visiblePresets = $derived(FRAMEWORK_PRESETS.filter((preset) => preset.mode === mode));
    const usesPackageManager = $derived(FRAMEWORK_PRESETS.find((preset) => preset.framework === framework)?.install_command != null);
    const canInspect = $derived(repoUrl.trim().startsWith("http") && !inspecting);
    const canCreate = $derived(name.trim() !== "" && inspected && !creating);

    $effect(() => {
        breadcrumb.set(rg.group ? `${rg.group.name} / Add web app` : null);
        return () => breadcrumb.set(null);
    });

    async function loadTokenNames() {
        tokenSecret = "";
        tokenNames = [];
        if (!auth.token || !vaultId) return;
        const secrets = await listSecrets(auth.token, rg.rgId, vaultId).catch(() => []);
        tokenNames = secrets.filter((secret) => !secret.deleted).map((secret) => secret.name);
    }

    function applyPreset(preset: Detection) {
        mode = preset.mode;
        installCommand = preset.install_command ?? "";
        buildCommand = preset.build_command ?? "";
        outputDir = preset.output_dir ?? "";
        startCommand = preset.start_command ?? "";
        containerPort = preset.port ?? DEFAULT_PORT;
    }

    function applyFramework() {
        const preset = FRAMEWORK_PRESETS.find((candidate) => candidate.framework === framework);
        if (preset) applyPreset(withPackageManager(preset, packageManager));
    }

    function selectMode(next: SiteMode) {
        mode = next;
        const current = FRAMEWORK_PRESETS.find((preset) => preset.framework === framework);
        if (current && current.mode !== next) framework = "";
    }

    async function useGateway(id: string) {
        await rg.load();
        gatewayId = id;
    }

    function selectFramework(id: string) {
        framework = id;
        applyFramework();
    }

    function selectPackageManager(manager: PackageManager) {
        packageManager = manager;
        applyFramework();
    }

    function applyDetection(found: Detection | null) {
        detection = found;
        if (!found) return;
        applyPreset(found);
        packageManager = detectPackageManager(found.install_command);
        framework = FRAMEWORK_PRESETS.some((preset) => preset.framework === found.framework) ? (found.framework ?? "") : "";
        if (!name) name = repoUrl.split("/").pop()?.replace(/\.git$/, "").toLowerCase().replace(/[^a-z0-9-]/g, "-") ?? "";
    }

    async function inspect() {
        if (!auth.token) return;
        const run = ++inspectRun;
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
            if (run !== inspectRun) return;
            applyDetection(result.detection);
            inspected = true;
        } catch (e) {
            if (run === inspectRun) error = e instanceof Error ? e.message : "Failed to inspect the repository";
        } finally {
            if (run === inspectRun) inspecting = false;
        }
    }

    $effect(() => {
        void [repoUrl, branch, rootDir, vaultId, tokenSecret];
        inspectRun++;
        inspected = false;
        inspecting = false;
    });

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
                gateway_id: gatewayId || undefined,
                preview_enabled: mode === "static" && previews,
                mode,
                framework: optional(framework),
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
                    <input class="cx-input" id="wa-repo" placeholder="https://github.com/org/repo" autocomplete="off" bind:value={repoUrl} />
                </div>
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                    <div class="cx-field">
                        <label class="cx-label" for="wa-branch">Production branch</label>
                        <input class="cx-input" id="wa-branch" autocomplete="off" bind:value={branch} />
                    </div>
                    <div class="cx-field">
                        <label class="cx-label" for="wa-root">Root directory</label>
                        <input class="cx-input" id="wa-root" placeholder="repository root" autocomplete="off" bind:value={rootDir} />
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
                        <select class="cx-input" id="wa-token" bind:value={tokenSecret} disabled={tokenNames.length === 0}>
                            <option value="">Select</option>
                            {#each tokenNames as secretName (secretName)}<option value={secretName}>{secretName}</option>{/each}
                        </select>
                    </div>
                </div>
                <div>
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={!canInspect} onclick={inspect}>
                        {inspected ? "Pull again" : "Pull repository"}
                    </button>
                </div>
                {#if inspecting}
                    <p class="cx-hint" aria-live="polite">Inspecting repository</p>
                {:else if inspected}
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
                        <button class="cx-tab" role="tab" type="button" aria-selected={mode === "static"} onclick={() => selectMode("static")}>Static site</button>
                        <button class="cx-tab" role="tab" type="button" aria-selected={mode === "container"} onclick={() => selectMode("container")}>Container app</button>
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
                            <label class="cx-label" for="wa-gateway">API gateway (optional)</label>
                            <select class="cx-input" id="wa-gateway" bind:value={gatewayId}>
                                <option value="">None, served inside the resource group</option>
                                {#each rg.gateways as gateway (gateway.id)}<option value={gateway.id}>{gateway.name}</option>{/each}
                            </select>
                            <p class="cx-hint">The site always gets an internal domain in this group. A gateway adds public hostnames and TLS.</p>
                            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => gatewayDialog?.open()}>Create API gateway</button>
                        </div>
                    </div>
                    <div class="cx-field" role="group" aria-labelledby="wa-framework">
                        <span class="cx-label" id="wa-framework">Framework</span>
                        <div style="display: flex; flex-wrap: wrap; gap: 8px">
                            {#each visiblePresets as preset (preset.framework)}
                                <button class="cx-btn cx-btn--sm {framework === preset.framework ? 'cx-btn--primary' : 'cx-btn--secondary'}" type="button" aria-pressed={framework === preset.framework} onclick={() => selectFramework(preset.framework)}>
                                    {preset.label}
                                </button>
                            {/each}
                            <button class="cx-btn cx-btn--sm {framework === '' ? 'cx-btn--primary' : 'cx-btn--secondary'}" type="button" aria-pressed={framework === ""} onclick={() => selectFramework("")}>Custom</button>
                        </div>
                        <p class="cx-hint">Detected from the repository. Choosing another resets the build settings.</p>
                    </div>
                    {#if usesPackageManager}
                        <div class="cx-field" role="group" aria-labelledby="wa-manager">
                            <span class="cx-label" id="wa-manager">Package manager</span>
                            <div style="display: flex; flex-wrap: wrap; gap: 8px">
                                {#each PACKAGE_MANAGERS as manager (manager)}
                                    <button class="cx-btn cx-btn--sm {packageManager === manager ? 'cx-btn--primary' : 'cx-btn--secondary'}" type="button" aria-pressed={packageManager === manager} onclick={() => selectPackageManager(manager)}>
                                        {manager}
                                    </button>
                                {/each}
                            </div>
                        </div>
                    {/if}
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
                            <p class="cx-hint">Static sites need no start command or container. The built files are served as they are.</p>
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

<CreateGatewayDialog bind:this={gatewayDialog} rgId={rg.rgId} onCreated={useGateway} />
