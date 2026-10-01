<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import {
        createWorkload,
        type PortMapping,
        type Volume,
        type VolumeMount,
    } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import Icon from "@iconify/svelte";

    let {
        rgId,
        volumes,
        onDeployed,
    }: { rgId: string; volumes: Volume[]; onDeployed: () => Promise<void> } = $props();

    let dialog = $state<HTMLDialogElement | null>(null);
    let deploying = $state(false);
    let deployError = $state<string | null>(null);

    let formImage = $state("");
    let formName = $state("");
    let formCpu = $state("500");
    let formMemory = $state("512");
    let formDisk = $state("1024");
    let formEnv = $state("");
    let formPortRows = $state<
        { containerPort: string; rgPort: string; nodePort: string; protocol: "tcp" | "udp" }[]
    >([]);
    let formVolumeMounts = $state("");

    export function open() {
        dialog?.showModal();
    }

    function parseEnvVars(raw: string): Record<string, string> | null {
        if (!raw.trim()) return null;
        const result: Record<string, string> = {};
        for (const line of raw.trim().split("\n")) {
            const eq = line.indexOf("=");
            if (eq === -1) continue;
            result[line.slice(0, eq).trim()] = line.slice(eq + 1).trim();
        }
        return Object.keys(result).length ? result : null;
    }

    function addPortRow() {
        formPortRows = [...formPortRows, { containerPort: "", rgPort: "", nodePort: "", protocol: "tcp" }];
    }

    function removePortRow(index: number) {
        formPortRows = formPortRows.filter((_, i) => i !== index);
    }

    function buildPortMappings(): PortMapping[] | null {
        const result: PortMapping[] = [];
        for (const row of formPortRows) {
            const containerPort = parseInt(String(row.containerPort).trim());
            if (!containerPort) continue;
            const rgPortTrimmed = String(row.rgPort).trim();
            const nodePortTrimmed = String(row.nodePort).trim();
            result.push({
                container_port: containerPort,
                protocol: row.protocol,
                rg_port: rgPortTrimmed ? parseInt(rgPortTrimmed) : null,
                node_port: nodePortTrimmed ? parseInt(nodePortTrimmed) : null,
            });
        }
        return result.length ? result : null;
    }

    function parseVolumeMounts(raw: string): VolumeMount[] | null {
        if (!raw.trim()) return null;
        const result: VolumeMount[] = [];
        for (const line of raw.trim().split("\n")) {
            const parts = line.trim().split(":");
            if (parts.length < 2) continue;
            const volumeName = parts[0].trim();
            const mountPath = parts.slice(1).join(":").trim();
            const vol = volumes.find((v) => v.name === volumeName || v.id === volumeName);
            if (!vol || !mountPath) continue;
            result.push({ volume_id: vol.id, mount_path: mountPath });
        }
        return result.length ? result : null;
    }

    async function handleDeploy() {
        if (!auth.token || !formImage || !formName) return;
        deploying = true;
        deployError = null;
        try {
            await createWorkload(auth.token, {
                name: formName,
                image: formImage,
                cpu_millicores: parseInt(formCpu),
                memory_bytes: parseInt(formMemory) * 1024 * 1024,
                disk_bytes: parseInt(formDisk) * 1024 * 1024,
                env_vars: parseEnvVars(formEnv),
                ports: buildPortMappings(),
                volume_mounts: parseVolumeMounts(formVolumeMounts),
                resource_group_id: rgId,
            });
            dialog?.close();
            resetDeployForm();
            await onDeployed();
        } catch (e) {
            deployError = e instanceof Error ? e.message : "Failed to deploy";
        } finally {
            deploying = false;
        }
    }

    function resetDeployForm() {
        formImage = "";
        formName = "";
        formCpu = "500";
        formMemory = "512";
        formDisk = "1024";
        formEnv = "";
        formPortRows = [];
        formVolumeMounts = "";
        deployError = null;
    }
</script>

<dialog
    bind:this={dialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-lg rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => resetDeployForm()}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Deploy Container</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => dialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="d-name">Name</label>
                <input id="d-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="my-app" bind:value={formName} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="d-image">Image</label>
                <input id="d-image" class="border rounded px-3 py-1.5 text-sm bg-background font-mono" placeholder="nginx:latest" bind:value={formImage} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="d-cpu">CPU (millicores)</label>
                <input id="d-cpu" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="500" bind:value={formCpu} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="d-mem">Memory (MB)</label>
                <input id="d-mem" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="512" bind:value={formMemory} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="d-disk">Disk (MB)</label>
                <input id="d-disk" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="1024" bind:value={formDisk} />
            </div>
            <div class="flex flex-col gap-2 sm:col-span-2">
                <div class="flex items-center justify-between">
                    <span class="text-xs text-muted-foreground">Ports</span>
                    <button
                        type="button"
                        class="text-xs font-medium text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1"
                        onclick={addPortRow}
                    >
                        <Icon icon="mdi:plus" width={14} height={14} />
                        Add port
                    </button>
                </div>
                {#if formPortRows.length === 0}
                    <p class="text-xs text-muted-foreground">No ports exposed. Container is only reachable via RG-internal DNS on its default port.</p>
                {:else}
                    <div class="flex items-center gap-3 pl-0.5">
                        <span class="text-xs text-muted-foreground w-20">Container port</span>
                        <span class="w-4 shrink-0"></span>
                        <span class="text-xs text-muted-foreground w-20">RG port</span>
                        <span class="w-px h-3 bg-border shrink-0"></span>
                        <span class="text-xs text-muted-foreground w-20">Node port</span>
                    </div>
                    <div class="space-y-2">
                        {#each formPortRows as row, i}
                            {@const cPort = String(row.containerPort).trim()}
                            {@const rPort = String(row.rgPort).trim()}
                            {@const nPort = String(row.nodePort).trim()}
                            <div class="flex items-center gap-3">
                                <input
                                    type="number"
                                    class="border rounded px-2 py-1.5 text-sm bg-background font-mono w-20"
                                    placeholder="80"
                                    aria-label="Container port"
                                    bind:value={row.containerPort}
                                />
                                <Icon icon="mdi:arrow-right" width={16} height={16} class="text-muted-foreground shrink-0" />
                                <input
                                    type="number"
                                    class="border rounded px-2 py-1.5 text-sm bg-background font-mono w-20"
                                    placeholder="8080"
                                    aria-label="RG port"
                                    bind:value={row.rgPort}
                                />
                                <span class="w-px h-5 bg-border shrink-0"></span>
                                <input
                                    type="number"
                                    class="border rounded px-2 py-1.5 text-sm bg-background font-mono w-20"
                                    placeholder="35000"
                                    aria-label="Node port"
                                    bind:value={row.nodePort}
                                />
                                <select
                                    class="border rounded px-2 py-1.5 text-sm bg-background"
                                    aria-label="Protocol"
                                    bind:value={row.protocol}
                                >
                                    <option value="tcp">TCP</option>
                                    <option value="udp">UDP</option>
                                </select>
                                <button
                                    type="button"
                                    class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors shrink-0"
                                    onclick={() => removePortRow(i)}
                                    aria-label="Remove port"
                                    title="Remove"
                                >
                                    <Icon icon="mdi:close" width={16} height={16} />
                                </button>
                            </div>
                            {#if cPort}
                                <p class="text-xs text-muted-foreground font-mono pl-0.5">
                                    RG mesh: {rPort || cPort}/{row.protocol} → container:{cPort}
                                    {#if nPort}
                                        &nbsp;·&nbsp; external: node-ip:{nPort} → container:{cPort}
                                    {/if}
                                </p>
                            {/if}
                        {/each}
                    </div>
                    <p class="text-xs text-muted-foreground">
                        RG port maps to the container port inside the RG mesh (optional, defaults to container port). Node port separately exposes it externally on the node's IP, like a Kubernetes NodePort.
                    </p>
                {/if}
            </div>
        </div>
        {#if volumes.length > 0}
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="d-vols">
                    Volume Mounts (name-or-id:/mount/path, one per line)
                </label>
                <textarea
                    id="d-vols"
                    class="border rounded px-3 py-1.5 text-sm bg-background font-mono resize-none"
                    rows={Math.min(4, volumes.length + 1)}
                    placeholder={volumes.map(v => `${v.name}:/data`).slice(0, 2).join("\n")}
                    bind:value={formVolumeMounts}
                ></textarea>
                <p class="text-xs text-muted-foreground">Available: {volumes.map(v => v.name).join(", ")}</p>
            </div>
        {/if}
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="d-env">Environment Variables (KEY=VALUE, one per line)</label>
            <textarea id="d-env" class="border rounded px-3 py-1.5 text-sm bg-background font-mono resize-none" rows={3} placeholder={"NODE_ENV=production\nPORT=3000"} bind:value={formEnv}></textarea>
        </div>
        {#if deployError}
            <p class="text-xs text-destructive">{deployError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleDeploy} disabled={deploying || !formName || !formImage}>
                {deploying ? "Deploying..." : "Deploy"}
            </Button>
        </div>
    </div>
</dialog>
