<script lang="ts">
    import {
        createWorkload,
        type PortMapping,
        type Volume,
        type VolumeMount,
    } from "$lib/api/resource-groups";
    import { ArrowRight, Plus, X } from "@lucide/svelte";
    import { Select } from "$lib/components/ui/select";

    let {
        rgId,
        volumes,
        onDeployed,
        oncancel,
    }: { rgId: string; volumes: Volume[]; onDeployed: () => Promise<void>; oncancel: () => void } = $props();

    let deploying = $state(false);
    let deployError = $state<string | null>(null);

    let formImage = $state("");
    let formName = $state("");
    let formCpu = $state("500");
    let formMemory = $state("512");
    let formDisk = $state("1024");
    let formMode = $state<"fixed" | "burst" | "auto">("fixed");
    let formCpuLimit = $state("1000");
    let formMemoryLimit = $state("1024");
    let formReplicas = $state("1");
    let formEnv = $state("");
    let formPortRows = $state<
        { containerPort: string; rgPort: string; nodePort: string; protocol: "tcp" | "udp" }[]
    >([]);
    let formVolumeMounts = $state("");

    const limitInvalid = $derived(
        formMode === "burst" &&
            (parseInt(formCpuLimit) < parseInt(formCpu) || parseInt(formMemoryLimit) < parseInt(formMemory)),
    );

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
        if (!formImage || !formName) return;
        deploying = true;
        deployError = null;
        try {
            await createWorkload({
                name: formName,
                image: formImage,
                cpu_millicores: parseInt(formCpu),
                memory_bytes: parseInt(formMemory) * 1024 * 1024,
                disk_bytes: parseInt(formDisk) * 1024 * 1024,
                cpu_limit_millicores: formMode === "burst" ? parseInt(formCpuLimit) : undefined,
                memory_limit_bytes: formMode === "burst" ? parseInt(formMemoryLimit) * 1024 * 1024 : undefined,
                auto_resources: formMode === "auto",
                replicas: parseInt(formReplicas) > 1 ? parseInt(formReplicas) : undefined,
                env_vars: parseEnvVars(formEnv),
                ports: buildPortMappings(),
                volume_mounts: parseVolumeMounts(formVolumeMounts),
                resource_group_id: rgId,
            });
            await onDeployed();
        } catch (e) {
            deployError = e instanceof Error ? e.message : "Failed to deploy";
        } finally {
            deploying = false;
        }
    }

</script>

<div class="md__fields md__fields--two">
    <div class="cx-field">
        <label class="cx-label" for="d-name">Name</label>
        <input id="d-name" class="cx-input" placeholder="my-app" bind:value={formName} />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="d-image">Image</label>
        <input id="d-image" class="cx-input cx-mono" placeholder="nginx:latest" bind:value={formImage} />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="d-mode">Resources</label>
        <Select
            id="d-mode"
            bind:value={formMode}
            options={[
                { value: "fixed", label: "Fixed" },
                { value: "burst", label: "Burst up to a limit" },
                { value: "auto", label: "Auto (right-size from usage)" },
            ]}
        />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="d-replicas">Replicas</label>
        <input id="d-replicas" type="number" min="1" max="32" class="cx-input" bind:value={formReplicas} />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="d-cpu">{formMode === "fixed" ? "CPU (millicores)" : "CPU request (millicores)"}</label>
        <input id="d-cpu" type="number" class="cx-input" placeholder="500" bind:value={formCpu} />
    </div>
    <div class="cx-field">
        <label class="cx-label" for="d-mem">{formMode === "fixed" ? "Memory (MB)" : "Memory request (MB)"}</label>
        <input id="d-mem" type="number" class="cx-input" placeholder="512" bind:value={formMemory} />
    </div>
    {#if formMode === "burst"}
        <div class="cx-field">
            <label class="cx-label" for="d-cpu-limit">CPU limit (millicores)</label>
            <input id="d-cpu-limit" type="number" class="cx-input" bind:value={formCpuLimit} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="d-mem-limit">Memory limit (MB)</label>
            <input id="d-mem-limit" type="number" class="cx-input" bind:value={formMemoryLimit} />
        </div>
        {#if limitInvalid}<p class="cx-hint cx-hint--error md__span">Limits must not be below the requests.</p>{/if}
    {:else if formMode === "auto"}
        <p class="cx-hint md__span">
            Starts with the values above. On the next restart the request shrinks to the observed peak plus 20 percent, and the original value becomes the burst limit.
        </p>
    {/if}
    <div class="cx-field">
        <label class="cx-label" for="d-disk">Disk (MB)</label>
        <input id="d-disk" type="number" class="cx-input" placeholder="1024" bind:value={formDisk} />
    </div>
    <div class="cx-field md__span">
        <div class="md__foot" style="justify-content: space-between; align-items: center">
            <span class="cx-label">Ports</span>
            <button type="button" class="cx-btn cx-btn--ghost cx-btn--sm" onclick={addPortRow}><Plus class="cx-i" aria-hidden="true" /> Add port</button>
        </div>
        {#if formPortRows.length === 0}
            <p class="cx-hint">No ports exposed. Container is only reachable via RG-internal DNS on its default port.</p>
        {:else}
            <div class="md__fields">
                {#each formPortRows as row, i}
                    {@const cPort = String(row.containerPort).trim()}
                    {@const rPort = String(row.rgPort).trim()}
                    {@const nPort = String(row.nodePort).trim()}
                    <div style="display: grid; grid-template-columns: 1fr auto 1fr 1fr auto auto; gap: var(--space-2); align-items: center">
                        <input type="number" class="cx-input cx-mono" placeholder="80" aria-label="Container port" bind:value={row.containerPort} />
                        <ArrowRight class="cx-i" aria-hidden="true" />
                        <input type="number" class="cx-input cx-mono" placeholder="8080" aria-label="RG port" bind:value={row.rgPort} />
                        <input type="number" class="cx-input cx-mono" placeholder="35000" aria-label="Node port" bind:value={row.nodePort} />
                        <Select
                            bind:value={row.protocol}
                            options={[
                                { value: "tcp", label: "TCP" },
                                { value: "udp", label: "UDP" },
                            ]}
                        />
                        <button type="button" class="cx-icon-btn cx-icon-btn--sm" onclick={() => removePortRow(i)} aria-label="Remove port" title="Remove">
                            <X class="cx-i" aria-hidden="true" />
                        </button>
                    </div>
                    {#if cPort}
                        <p class="cx-hint cx-mono">
                            RG mesh: {rPort || cPort}/{row.protocol} → container:{cPort}
                            {#if nPort}&nbsp;·&nbsp; external: node-ip:{nPort} → container:{cPort}{/if}
                        </p>
                    {/if}
                {/each}
            </div>
            <p class="cx-hint">
                Order: container port, RG port, node port. RG port maps to the container port inside the RG mesh (optional, defaults to container port). Node port separately exposes it externally on the node's IP.
            </p>
        {/if}
    </div>
    {#if volumes.length > 0}
        <div class="cx-field md__span">
            <label class="cx-label" for="d-vols">Volume Mounts (name-or-id:/mount/path, one per line)</label>
            <textarea
                id="d-vols"
                class="cx-input cx-mono"
                style="height: auto; padding: 10px 14px"
                rows={Math.min(4, volumes.length + 1)}
                placeholder={volumes.map((v) => `${v.name}:/data`).slice(0, 2).join("\n")}
                bind:value={formVolumeMounts}
            ></textarea>
            <p class="cx-hint">Available: {volumes.map((v) => v.name).join(", ")}</p>
        </div>
    {/if}
    <div class="cx-field md__span">
        <label class="cx-label" for="d-env">Environment Variables (KEY=VALUE, one per line)</label>
        <textarea id="d-env" class="cx-input cx-mono" style="height: auto; padding: 10px 14px" rows={3} placeholder={"NODE_ENV=production\nDB_PASSWORD=vault://<vault-id>/DB_PASSWORD"} aria-describedby="d-env-hint" bind:value={formEnv}></textarea>
        <p class="cx-hint" id="d-env-hint">Store secrets in a vault and reference them as vault://&lt;vault-id&gt;/&lt;secret-name&gt;. The value is resolved when the workload starts. Plain values are stored encrypted and shown masked.</p>
    </div>
    {#if deployError}<p class="cx-hint cx-hint--error md__span">{deployError}</p>{/if}
    <div class="md__foot md__span">
        <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={oncancel}>Cancel</button>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={handleDeploy} disabled={deploying || !formName || !formImage || limitInvalid} aria-busy={deploying}>
            {deploying ? "Deploying" : "Deploy"}
        </button>
    </div>
</div>
