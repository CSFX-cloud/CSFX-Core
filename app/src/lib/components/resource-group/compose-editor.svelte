<script lang="ts">
    import { onMount } from "svelte";
    import { Upload } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { createWorkloadStack, getStack, redeployStack } from "$lib/api/resource-groups";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { parseComposePreview } from "$lib/utils/compose-preview";
    import { knownWorkloadIcon } from "$lib/utils/image-icon";
    import { highlightYaml } from "$lib/utils/yaml-highlight";

    const STACK_NAME_PATTERN = /^[a-z0-9][a-z0-9-]*$/;
    const PLACEHOLDER = 'services:\n  redis:\n    image: redis:7\n  web:\n    image: nginx:alpine\n    ports:\n      - "8080:80"';

    type Props = {
        rgId: string;
        groupName: string;
        stackId?: string | null;
        onDone: () => Promise<void> | void;
        oncancel?: () => void;
    };

    let { rgId, groupName, stackId = null, onDone, oncancel }: Props = $props();

    let deploying = $state(false);
    let error = $state<string | null>(null);
    let stackName = $state("");
    let yaml = $state("");
    let gutter = $state<HTMLDivElement | null>(null);
    let textarea = $state<HTMLTextAreaElement | null>(null);
    let layer = $state<HTMLPreElement | null>(null);

    const editing = $derived(stackId !== null);
    const preview = $derived(parseComposePreview(yaml));
    const lineCount = $derived(Math.max(yaml.split("\n").length, 1));
    const highlighted = $derived(highlightYaml(yaml));
    const publishedPorts = $derived(preview.services.reduce((sum, service) => sum + service.ports.filter(isPublic).length, 0));
    const nameValid = $derived(editing || STACK_NAME_PATTERN.test(stackName));
    const valid = $derived(yaml.trim() !== "" && preview.error === null && preview.services.length > 0);

    function isPublic(port: string): boolean {
        return port.includes(":");
    }

    function portLabel(port: string): string {
        const [host, container] = port.split(":");
        return `Public · ${host} → ${container}`;
    }

    function syncScroll() {
        if (!textarea) return;
        if (gutter) gutter.scrollTop = textarea.scrollTop;
        if (layer) {
            layer.scrollTop = textarea.scrollTop;
            layer.scrollLeft = textarea.scrollLeft;
        }
    }

    async function loadStack(id: string) {
        if (!auth.token) return;
        error = null;
        try {
            const stack = await getStack(auth.token, id);
            stackName = stack.name;
            yaml = stack.compose_source ?? "";
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load stack";
        }
    }

    async function deploy() {
        if (!auth.token || !valid || !nameValid || (!editing && !stackName)) return;
        deploying = true;
        error = null;
        try {
            if (stackId) {
                await redeployStack(auth.token, stackId, yaml);
            } else {
                await createWorkloadStack(auth.token, { name: stackName, resource_group_id: rgId, compose_yaml: yaml });
            }
            await onDone();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to deploy stack";
        } finally {
            deploying = false;
        }
    }

    function upload(event: Event) {
        const file = (event.target as HTMLInputElement).files?.[0];
        if (file) file.text().then((text) => (yaml = text));
    }

    onMount(() => {
        if (stackId) loadStack(stackId);
    });
</script>

<div style="display: grid; gap: 16px">
    {#if !editing}
        <div class="cx-field" style="max-width: 520px">
            <label class="cx-label" for="cp-name">Stack name</label>
            <input class="cx-input" id="cp-name" type="text" placeholder="my-stack" autocomplete="off" aria-describedby="cp-name-hint" aria-invalid={stackName !== "" && !nameValid} bind:value={stackName} />
            <p class="cx-hint" id="cp-name-hint">Lowercase letters, numbers and dashes. Containers are named after it.</p>
        </div>
    {/if}

    <div class="cp-grid">
        <section class="cx-panel cp-editor" aria-labelledby="cp-file">
            <div class="cp-editor__bar">
                <b id="cp-file">docker-compose.yml</b>
                <label class="cx-btn cx-btn--secondary cx-btn--sm" style="cursor: pointer">
                    <Upload class="cx-i" aria-hidden="true" />
                    Upload file
                    <input type="file" accept=".yml,.yaml" class="db-sr-only" onchange={upload} />
                </label>
            </div>
            <div class="cp-code">
                <div class="cp-code__nums" bind:this={gutter} aria-hidden="true">
                    {#each { length: lineCount } as _, index (index)}<div>{index + 1}</div>{/each}
                </div>
                <pre class="cp-code__layer" bind:this={layer}>{@html highlighted}</pre>
                <textarea class="cp-code__input" aria-label="Compose file" spellcheck="false" placeholder={PLACEHOLDER} bind:this={textarea} onscroll={syncScroll} bind:value={yaml}></textarea>
            </div>
        </section>

        <section class="cx-panel" aria-labelledby="cp-prev">
            <div class="cx-panel__head">
                <div><h2 class="cx-panel__title" id="cp-prev">Preview</h2><p class="cx-panel__sub">What csfx.cloud will create</p></div>
                {#if yaml.trim() === ""}
                    <span class="cx-badge">Empty</span>
                {:else if valid}
                    <span class="cx-badge cx-badge--success">Valid</span>
                {:else}
                    <span class="cx-badge cx-badge--danger">Invalid</span>
                {/if}
            </div>
            {#if preview.error && yaml.trim() !== ""}<p class="cx-hint cx-hint--error">{preview.error}</p>{/if}
            <div>
                {#each preview.services as service (service.serviceName)}
                    <div class="cp-svc">
                        <ServiceIcon name="docker-containers" brand={knownWorkloadIcon(service.image ?? "", "firecracker")} />
                        <div>
                            <b>{service.serviceName}</b>
                            <code>{service.image ?? "no image"}</code>
                            <div class="cp-svc__ports">
                                {#each service.ports.filter(isPublic) as port (port)}
                                    <span class="cx-badge" style="color: var(--ink)">{portLabel(port)}</span>
                                {:else}
                                    <span class="cx-badge">Internal only</span>
                                {/each}
                            </div>
                        </div>
                    </div>
                {:else}
                    <p class="cx-hint">No services detected yet.</p>
                {/each}
            </div>
            <dl class="nd-kv" style="margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--border-100)">
                <div><dt>Containers</dt><dd class="mono">{preview.services.length}</dd></div>
                <div><dt>Published ports</dt><dd class="mono">{publishedPorts}</dd></div>
                <div><dt>Volumes</dt><dd class="mono">{preview.volumes.length}</dd></div>
            </dl>
        </section>
    </div>

    {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

    <div class="cp-foot">
        <p>Every service joins {groupName}'s network and reaches the others by its service name.</p>
        <div class="db-actions">
            {#if oncancel}<button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={oncancel}>Cancel</button>{/if}
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={deploying || !valid || !nameValid || (!editing && !stackName)} aria-busy={deploying} onclick={deploy}>
                {#if deploying}{editing ? "Redeploying" : "Deploying"}{:else}{editing ? "Redeploy" : "Deploy stack"}{/if}
            </button>
        </div>
    </div>
</div>
