<script lang="ts">
    import { untrack } from "svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { updateSite, type SiteDetail } from "$lib/api/sites";

    type Props = { rgId: string; site: SiteDetail; onChanged: () => Promise<void>; onError: (message: string) => void };

    let { rgId, site, onChanged, onError }: Props = $props();

    const initial = untrack(() => site);

    let branch = $state(initial.production_branch);
    let rootDir = $state(initial.root_dir);
    let installCommand = $state(initial.install_command ?? "");
    let buildCommand = $state(initial.build_command ?? "");
    let outputDir = $state(initial.output_dir ?? "");
    let startCommand = $state(initial.start_command ?? "");
    let containerPort = $state(initial.container_port ?? 3000);
    let previews = $state(initial.preview_enabled);
    let saving = $state(false);

    async function save() {
        if (!auth.token) return;
        saving = true;
        try {
            await updateSite(auth.token, rgId, site.id, {
                production_branch: branch.trim(),
                root_dir: rootDir.trim(),
                install_command: installCommand,
                build_command: buildCommand,
                output_dir: site.mode === "static" ? outputDir : undefined,
                start_command: site.mode === "container" ? startCommand : undefined,
                container_port: site.mode === "container" ? containerPort : undefined,
                preview_enabled: previews,
            });
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to save settings");
        } finally {
            saving = false;
        }
    }
</script>

<section class="cx-panel" aria-labelledby="bp-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="bp-title">Build and git</h2>
            <p class="cx-panel__sub">Detected automatically from {site.repo_url.replace(/^https?:\/\//, "")}. Change what you need.</p>
        </div>
    </div>
    <form
        class="md__fields"
        style="padding: 0 24px 24px"
        onsubmit={(event) => {
            event.preventDefault();
            save();
        }}
    >
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
            <div class="cx-field">
                <label class="cx-label" for="bp-branch">Production branch</label>
                <input class="cx-input" id="bp-branch" autocomplete="off" bind:value={branch} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="bp-root">Root directory</label>
                <input class="cx-input" id="bp-root" autocomplete="off" bind:value={rootDir} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="bp-install">Install command</label>
                <input class="cx-input" id="bp-install" autocomplete="off" bind:value={installCommand} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="bp-build">Build command</label>
                <input class="cx-input" id="bp-build" autocomplete="off" bind:value={buildCommand} />
            </div>
            {#if site.mode === "static"}
                <div class="cx-field">
                    <label class="cx-label" for="bp-output">Output directory</label>
                    <input class="cx-input" id="bp-output" autocomplete="off" bind:value={outputDir} />
                </div>
            {:else}
                <div class="cx-field">
                    <label class="cx-label" for="bp-start">Start command</label>
                    <input class="cx-input" id="bp-start" autocomplete="off" bind:value={startCommand} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="bp-port">Port</label>
                    <input class="cx-input" id="bp-port" type="number" min="1" max="65535" bind:value={containerPort} />
                </div>
            {/if}
        </div>
        {#if site.mode === "static"}
            <label class="cx-check"><input type="checkbox" bind:checked={previews} /> Preview every pull request</label>
        {/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving} aria-busy={saving}>{saving ? "Saving" : "Save"}</button>
        </div>
    </form>
</section>
