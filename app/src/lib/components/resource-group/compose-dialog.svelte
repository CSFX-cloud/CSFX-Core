<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createWorkloadStack, getStack, redeployStack } from "$lib/api/resource-groups";
    import { parseComposePreview } from "$lib/utils/compose-preview";
    import { resolveImageIcon } from "$lib/utils/image-icon";
    import { highlightYaml } from "$lib/utils/yaml-highlight";
    import { Button } from "$lib/components/ui/button/index.js";
    import Icon from "@iconify/svelte";

    let { rgId, onDeployed }: { rgId: string; onDeployed: () => Promise<void> } = $props();

    let dialog = $state<HTMLDialogElement | null>(null);
    let deployingStack = $state(false);
    let composeError = $state<string | null>(null);
    let composeStackName = $state("");
    let composeYaml = $state("");
    let editingStackId = $state<string | null>(null);
    let composePreview = $derived(parseComposePreview(composeYaml));
    let composeLineCount = $derived(Math.max(composeYaml.split("\n").length, 1));
    let composeGutter = $state<HTMLDivElement | null>(null);
    let composeTextarea = $state<HTMLTextAreaElement | null>(null);
    let composeHighlightLayer = $state<HTMLPreElement | null>(null);
    let composeHighlighted = $derived(highlightYaml(composeYaml));
    function syncComposeScroll() {
        if (composeGutter && composeTextarea) {
            composeGutter.scrollTop = composeTextarea.scrollTop;
        }
        if (composeHighlightLayer && composeTextarea) {
            composeHighlightLayer.scrollTop = composeTextarea.scrollTop;
            composeHighlightLayer.scrollLeft = composeTextarea.scrollLeft;
        }
    }

    export function open(stackId?: string) {
        dialog?.showModal();
        if (stackId) loadStack(stackId);
    }

    async function handleDeployStack() {
        if (!auth.token || !composeYaml.trim()) return;
        if (!editingStackId && !composeStackName) return;
        deployingStack = true;
        composeError = null;
        try {
            if (editingStackId) {
                await redeployStack(auth.token, editingStackId, composeYaml);
            } else {
                await createWorkloadStack(auth.token, {
                    name: composeStackName,
                    resource_group_id: rgId,
                    compose_yaml: composeYaml,
                });
            }
            dialog?.close();
            resetComposeForm();
            await onDeployed();
        } catch (e) {
            composeError = e instanceof Error ? e.message : "Failed to deploy stack";
        } finally {
            deployingStack = false;
        }
    }

    function resetComposeForm() {
        composeStackName = "";
        composeYaml = "";
        composeError = null;
        editingStackId = null;
    }

    async function loadStack(stackId: string) {
        if (!auth.token) return;
        composeError = null;
        editingStackId = stackId;
        composeStackName = "";
        composeYaml = "";
        try {
            const stack = await getStack(auth.token, stackId);
            composeStackName = stack.name;
            composeYaml = stack.compose_source ?? "";
        } catch (e) {
            composeError = e instanceof Error ? e.message : "Failed to load stack";
        }
    }

    function handleComposeFileUpload(event: Event) {
        const input = event.target as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        file.text().then((text) => {
            composeYaml = text;
        });
    }
</script>

<dialog
    bind:this={dialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-6xl rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => resetComposeForm()}
>
    <div class="flex flex-col gap-4 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">{editingStackId ? "Edit Compose Stack" : "Deploy Docker Compose Stack"}</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => dialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="c-name">Stack Name</label>
            <input id="c-name" class="border rounded px-3 py-1.5 text-sm bg-background disabled:opacity-60" placeholder="my-stack" bind:value={composeStackName} disabled={!!editingStackId} />
        </div>
        <div class="grid grid-cols-5 gap-4">
            <div class="col-span-2 flex flex-col gap-1">
                <label class="text-xs text-muted-foreground">Preview</label>
                <div class="border rounded flex flex-col gap-2 p-3 h-[32rem] overflow-y-auto bg-muted/20">
                    {#if composePreview.services.length === 0 && composePreview.volumes.length === 0}
                        <p class="text-xs text-muted-foreground">No services detected yet.</p>
                    {:else}
                        {#each composePreview.services as service (service.serviceName)}
                            <div class="flex items-center gap-2.5 rounded border bg-background p-2">
                                <Icon icon={resolveImageIcon(service.image ?? "")} width={20} height={20} class="shrink-0" />
                                <div class="min-w-0">
                                    <p class="text-sm font-medium leading-tight truncate">{service.serviceName}</p>
                                    <p class="text-xs text-muted-foreground font-mono truncate">{service.image ?? "no image"}</p>
                                    {#if service.ports.length > 0}
                                        <p class="text-xs text-muted-foreground font-mono truncate">{service.ports.join(", ")}</p>
                                    {/if}
                                </div>
                            </div>
                        {/each}
                        {#if composePreview.volumes.length > 0}
                            <p class="text-xs text-muted-foreground mt-1">Volumes</p>
                            {#each composePreview.volumes as volumeName (volumeName)}
                                <div class="flex items-center gap-2.5 rounded border bg-background p-2">
                                    <Icon icon="mdi:database-outline" width={20} height={20} class="shrink-0 text-muted-foreground" />
                                    <p class="text-sm font-medium leading-tight truncate">{volumeName}</p>
                                </div>
                            {/each}
                        {/if}
                    {/if}
                </div>
            </div>
            <div class="col-span-3 flex flex-col gap-1">
                <div class="flex items-center justify-between">
                    <label class="text-xs text-muted-foreground" for="c-yaml">docker-compose.yml</label>
                    <label class="text-xs text-primary cursor-pointer hover:underline">
                        Upload file
                        <input type="file" accept=".yml,.yaml" class="hidden" onchange={handleComposeFileUpload} />
                    </label>
                </div>
                <div class="relative border rounded h-[32rem] overflow-hidden bg-background">
                    <div
                        bind:this={composeGutter}
                        class="absolute left-0 top-0 bottom-0 w-9 overflow-hidden py-1.5 text-right text-xs leading-relaxed font-mono text-muted-foreground select-none bg-muted/30 border-r"
                    >
                        {#each Array(composeLineCount) as _, i}
                            <div class="px-1.5">{i + 1}</div>
                        {/each}
                    </div>
                    <pre
                        bind:this={composeHighlightLayer}
                        class="absolute inset-0 left-9 overflow-hidden pl-2 pr-3 py-1.5 text-xs leading-relaxed font-mono whitespace-pre-wrap break-words m-0 pointer-events-none"
                    >{@html composeHighlighted}</pre>
                    <textarea
                        id="c-yaml"
                        bind:this={composeTextarea}
                        onscroll={syncComposeScroll}
                        spellcheck="false"
                        class="absolute inset-0 pl-11 pr-3 py-1.5 text-xs leading-relaxed bg-transparent font-mono resize-none outline-none w-full h-full text-transparent caret-foreground placeholder:text-muted-foreground"
                        placeholder={"services:\n  redis:\n    image: redis:7\n  web:\n    image: nginx:alpine\n    ports:\n      - \"8080:80\""}
                        bind:value={composeYaml}
                    ></textarea>
                </div>
            </div>
        </div>
        {#if composeError}
            <p class="text-xs text-destructive">{composeError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleDeployStack} disabled={deployingStack || (!editingStackId && !composeStackName) || !composeYaml.trim()}>
                {#if deployingStack}
                    {editingStackId ? "Redeploying..." : "Deploying..."}
                {:else}
                    {editingStackId ? "Redeploy" : "Deploy Stack"}
                {/if}
            </Button>
        </div>
    </div>
</dialog>
