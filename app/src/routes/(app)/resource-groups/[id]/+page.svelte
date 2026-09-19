<script lang="ts">
    import { tick, onDestroy } from "svelte";
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { auth } from "$lib/auth/store.svelte";
    import {
        getResourceGroup,
        updateResourceGroup,
        deleteResourceGroup,
        uploadResourceGroupIconImage,
        deleteResourceGroupIconImage,
        resourceGroupIconImageUrl,
        listResourceGroupWorkloads,
        listResourceGroupVolumes,
        createWorkload,
        createWorkloadStack,
        getStack,
        deleteStack,
        stopStack,
        restartStack,
        redeployStack,
        deleteWorkload,
        stopWorkload,
        restartWorkload,
        updateWorkload,
        createVolume,
        deleteVolume,
        listResourceGroupBuckets,
        createBucket,
        deleteBucket,
        listBucketKeys,
        createBucketKey,
        streamWorkloadLogs,
        openWorkloadExecSocket,
        ensureIsoBucket,
        uploadIso,
        listBucketObjects,
        presignObjectDownload,
        type ResourceGroup,
        type Workload,
        type Volume,
        type Bucket,
        type BucketAccessKey,
        type PortMapping,
        type VolumeMount,
        type ObjectEntry,
    } from "$lib/api/resource-groups";
    import { getNode } from "$lib/api/nodes";
    import { resolveImageIcon } from "$lib/utils/image-icon";
    import { parseComposePreview } from "$lib/utils/compose-preview";
    import { highlightYaml } from "$lib/utils/yaml-highlight";
    import type { Terminal } from "@xterm/xterm";
    import type { FitAddon } from "@xterm/addon-fit";
    import "@xterm/xterm/css/xterm.css";
    import Icon from "@iconify/svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import IconPicker from "$lib/components/icon-picker.svelte";
    import RgIcon from "$lib/components/rg-icon.svelte";
    import { commandPalette } from "$lib/components/command-palette/command-palette-store.svelte.js";
    import SearchIcon from "@lucide/svelte/icons/search";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import VncConsole from "$lib/components/vnc-console.svelte";
    import { isTransientStatus } from "$lib/utils/status.js";

    const rgId: string = $page.params.id;

    let group = $state<ResourceGroup | null>(null);
    let workloads = $state<Workload[]>([]);
    let volumes = $state<Volume[]>([]);
    let buckets = $state<Bucket[]>([]);
    let loading = $state(true);
    let error = $state<string | null>(null);

    let appearanceDialog = $state<HTMLDialogElement | null>(null);
    let editIcon = $state("mdi:cube-outline");
    let editColor = $state("#6366f1");
    let savingAppearance = $state(false);
    let appearanceError = $state<string | null>(null);
    let uploadingIconImage = $state(false);
    let iconImageError = $state<string | null>(null);

    let rgSettingsDialog = $state<HTMLDialogElement | null>(null);
    let editName = $state("");
    let editDescription = $state("");
    let savingRgSettings = $state(false);
    let rgSettingsError = $state<string | null>(null);

    let activeTab = $state<"all" | "container" | "vm" | "volume" | "bucket">("all");
    let filterText = $state("");

    let nodeIpCache = $state<Record<string, string | null>>({});

    async function resolveNodeIp(agentId: string | null): Promise<string | null> {
        if (!agentId || !auth.token) return null;
        if (agentId in nodeIpCache) return nodeIpCache[agentId];
        try {
            const node = await getNode(auth.token, agentId);
            nodeIpCache = { ...nodeIpCache, [agentId]: node.ip_address };
            return node.ip_address;
        } catch {
            nodeIpCache = { ...nodeIpCache, [agentId]: null };
            return null;
        }
    }

    let copiedKey = $state<string | null>(null);

    async function copyToClipboard(value: string, key: string) {
        try {
            await navigator.clipboard.writeText(value);
            copiedKey = key;
            setTimeout(() => {
                if (copiedKey === key) copiedKey = null;
            }, 1500);
        } catch {
            copiedKey = null;
        }
    }

    let deployDialog = $state<HTMLDialogElement | null>(null);
    let volumeDialog = $state<HTMLDialogElement | null>(null);
    let bucketDialog = $state<HTMLDialogElement | null>(null);
    let bucketDetailDialog = $state<HTMLDialogElement | null>(null);
    let resourcePickerDialog = $state<HTMLDialogElement | null>(null);
    let composeDialog = $state<HTMLDialogElement | null>(null);
    let deploying = $state(false);
    let creatingVolume = $state(false);
    let creatingBucket = $state(false);
    let deployingStack = $state(false);
    let deployError = $state<string | null>(null);
    let volumeError = $state<string | null>(null);
    let bucketError = $state<string | null>(null);
    let composeError = $state<string | null>(null);
    let downloadingVpn = $state(false);
    let expandedStacks = $state<Set<string>>(new Set());

    const RESOURCE_TYPES = [
        { key: "docker-container", label: "Docker Container", description: "Deploy a single container", icon: "logos:docker-icon" },
        { key: "docker-compose", label: "Docker Compose", description: "Deploy multiple related containers as one stack", icon: "logos:docker-icon" },
        { key: "vm", label: "Virtual Machine", description: "Boot a full VM from an ISO image", icon: "mdi:monitor" },
        { key: "volume", label: "Volume", description: "Add a block storage volume", icon: "mdi:database-outline" },
        { key: "bucket", label: "S3 Bucket", description: "Add an S3-compatible object storage bucket", icon: "fluent-emoji-high-contrast:bucket" },
    ] as const;

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

    let vmDialog = $state<HTMLDialogElement | null>(null);
    let vmFormName = $state("");
    let vmFormVcpus = $state("2");
    let vmFormMemoryGb = $state("2");
    let vmFormDiskGb = $state("20");
    let vmFormAdvanced = $state(false);
    let vmFormCpuMillicores = $state("2000");
    let vmFormMemoryMb = $state("2048");
    let vmFormDiskMb = $state("20480");

    let vmEffectiveCpuMillicores = $derived(
        vmFormAdvanced ? parseInt(vmFormCpuMillicores) || 0 : (parseInt(vmFormVcpus) || 0) * 1000
    );
    let vmEffectiveMemoryMb = $derived(
        vmFormAdvanced ? parseInt(vmFormMemoryMb) || 0 : (parseInt(vmFormMemoryGb) || 0) * 1024
    );
    let vmEffectiveDiskMb = $derived(
        vmFormAdvanced ? parseInt(vmFormDiskMb) || 0 : (parseInt(vmFormDiskGb) || 0) * 1024
    );
    let vmFormIsoFile = $state<File | null>(null);
    let vmIsoMode = $state<"existing" | "upload">("upload");
    let vmExistingIsos = $state<ObjectEntry[]>([]);
    let vmExistingIsosLoading = $state(false);
    let vmSelectedIsoKey = $state("");
    let vmIsoBucketId = $state<string | null>(null);
    let vmDeploying = $state(false);
    let vmUploadProgress = $state(0);
    let vmError = $state<string | null>(null);

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

    let volFormName = $state("");
    let volFormSize = $state("10");

    let bucketFormName = $state("");
    let bucketFormExposure = $state<"internal" | "external">("internal");

    let activeBucket = $state<Bucket | null>(null);
    let bucketKeys = $state<BucketAccessKey[]>([]);
    let bucketKeysError = $state<string | null>(null);
    let newKeyName = $state("");
    let creatingKey = $state(false);
    let createdKeySecret = $state<string | null>(null);

    let containerDialog = $state<HTMLDialogElement | null>(null);
    let containerDialogTab = $state<"logs" | "shell" | "insights" | "network" | "settings">("logs");
    let activeContainer = $state<Workload | null>(null);
    let containerActionError = $state<string | null>(null);
    let containerActionBusy = $state(false);

    let settingsImage = $state("");
    let settingsEnvText = $state("");
    let settingsRestartPolicy = $state<"always" | "on-failure" | "never">("always");
    let settingsMaxRestarts = $state("");
    let settingsError = $state<string | null>(null);
    let settingsSaving = $state(false);

    let logsLines = $state<string[]>([]);
    let logsError = $state<string | null>(null);
    let logsAbort: AbortController | null = null;

    let execError = $state<string | null>(null);
    let execTerminalEl = $state<HTMLDivElement | null>(null);
    let execTerminal: Terminal | null = null;
    let execFitAddon: FitAddon | null = null;
    let execSocket: WebSocket | null = null;

    async function load() {
        if (!auth.token) return;
        try {
            [group, workloads, volumes, buckets] = await Promise.all([
                getResourceGroup(auth.token, rgId),
                listResourceGroupWorkloads(auth.token, rgId),
                listResourceGroupVolumes(auth.token, rgId),
                listResourceGroupBuckets(auth.token, rgId),
            ]);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load";
        } finally {
            loading = false;
        }
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
            deployDialog?.close();
            resetDeployForm();
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
        } catch (e) {
            deployError = e instanceof Error ? e.message : "Failed to deploy";
        } finally {
            deploying = false;
        }
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
            composeDialog?.close();
            resetComposeForm();
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
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

    async function openStackEditor(stackId: string) {
        if (!auth.token) return;
        composeError = null;
        editingStackId = stackId;
        composeStackName = "";
        composeYaml = "";
        composeDialog?.showModal();
        try {
            const stack = await getStack(auth.token, stackId);
            composeStackName = stack.name;
            composeYaml = stack.compose_source ?? "";
        } catch (e) {
            composeError = e instanceof Error ? e.message : "Failed to load stack";
        }
    }

    async function handleStopStack(stackId: string) {
        if (!auth.token) return;
        try {
            await stopStack(auth.token, stackId);
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to stop stack";
        }
    }

    async function handleRestartStack(stackId: string) {
        if (!auth.token) return;
        try {
            await restartStack(auth.token, stackId);
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to restart stack";
        }
    }

    async function handleDeleteStack(stackId: string) {
        if (!auth.token) return;
        try {
            await deleteStack(auth.token, stackId);
            workloads = workloads.filter((w) => w.stack_id !== stackId);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete stack";
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

    function openResourceType(key: (typeof RESOURCE_TYPES)[number]["key"]) {
        resourcePickerDialog?.close();
        if (key === "docker-container") {
            deployDialog?.showModal();
        } else if (key === "docker-compose") {
            composeDialog?.showModal();
        } else if (key === "vm") {
            vmDialog?.showModal();
            loadExistingIsos();
        } else if (key === "volume") {
            volumeDialog?.showModal();
        } else {
            bucketDialog?.showModal();
        }
    }

    async function loadExistingIsos() {
        if (!auth.token) return;
        vmExistingIsosLoading = true;
        try {
            const bucket = await ensureIsoBucket(auth.token, rgId);
            vmIsoBucketId = bucket.id;
            const result = await listBucketObjects(auth.token, bucket.id, "");
            vmExistingIsos = result.objects;
            if (vmExistingIsos.length > 0) {
                vmIsoMode = "existing";
                vmSelectedIsoKey = vmExistingIsos[0].key;
            } else {
                vmIsoMode = "upload";
            }
        } catch (e) {
            vmError = e instanceof Error ? e.message : "Failed to load existing isos";
        } finally {
            vmExistingIsosLoading = false;
        }
    }

    function isoFileName(key: string): string {
        const parts = key.split("-");
        return parts.length > 5 ? parts.slice(5).join("-") : key;
    }

    function resetVmForm() {
        vmFormName = "";
        vmFormVcpus = "2";
        vmFormMemoryGb = "2";
        vmFormDiskGb = "20";
        vmFormAdvanced = false;
        vmFormCpuMillicores = "2000";
        vmFormMemoryMb = "2048";
        vmFormDiskMb = "20480";
        vmFormIsoFile = null;
        vmIsoMode = "upload";
        vmExistingIsos = [];
        vmSelectedIsoKey = "";
        vmIsoBucketId = null;
        vmUploadProgress = 0;
        vmError = null;
    }

    function handleVmIsoFileChange(event: Event) {
        const input = event.target as HTMLInputElement;
        vmFormIsoFile = input.files?.[0] ?? null;
    }

    async function handleDeployVm() {
        if (!auth.token || !vmFormName) return;
        if (vmIsoMode === "upload" && !vmFormIsoFile) return;
        if (vmIsoMode === "existing" && !vmSelectedIsoKey) return;
        vmDeploying = true;
        vmUploadProgress = 0;
        vmError = null;
        try {
            let isoUrl: string;
            if (vmIsoMode === "existing" && vmIsoBucketId) {
                const presigned = await presignObjectDownload(auth.token, vmIsoBucketId, vmSelectedIsoKey);
                isoUrl = presigned.url;
            } else {
                const bucket = await ensureIsoBucket(auth.token, rgId);
                isoUrl = await uploadIso(auth.token, bucket.id, vmFormIsoFile!, (fraction) => {
                    vmUploadProgress = fraction;
                });
            }

            await createWorkload(auth.token, {
                name: vmFormName,
                image: isoUrl,
                cpu_millicores: vmEffectiveCpuMillicores,
                memory_bytes: vmEffectiveMemoryMb * 1024 * 1024,
                disk_bytes: vmEffectiveDiskMb * 1024 * 1024,
                env_vars: null,
                ports: null,
                volume_mounts: null,
                resource_group_id: rgId,
                runtime_class: "vm",
            });
            vmDialog?.close();
            resetVmForm();
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
        } catch (e) {
            vmError = e instanceof Error ? e.message : "Failed to deploy vm";
        } finally {
            vmDeploying = false;
        }
    }

    function toggleStack(stackId: string) {
        const next = new Set(expandedStacks);
        if (next.has(stackId)) {
            next.delete(stackId);
        } else {
            next.add(stackId);
        }
        expandedStacks = next;
    }

    async function handleCreateVolume() {
        if (!auth.token || !volFormName) return;
        creatingVolume = true;
        volumeError = null;
        try {
            await createVolume(auth.token, {
                name: volFormName,
                size_gb: parseInt(volFormSize),
                resource_group_id: rgId,
            });
            volumeDialog?.close();
            volFormName = "";
            volFormSize = "10";
            volumes = await listResourceGroupVolumes(auth.token, rgId);
        } catch (e) {
            volumeError = e instanceof Error ? e.message : "Failed to create volume";
        } finally {
            creatingVolume = false;
        }
    }

    async function handleCreateBucket() {
        if (!auth.token || !bucketFormName) return;
        creatingBucket = true;
        bucketError = null;
        try {
            await createBucket(auth.token, {
                name: bucketFormName,
                resource_group_id: rgId,
                exposure: bucketFormExposure,
            });
            bucketDialog?.close();
            bucketFormName = "";
            bucketFormExposure = "internal";
            buckets = await listResourceGroupBuckets(auth.token, rgId);
        } catch (e) {
            bucketError = e instanceof Error ? e.message : "Failed to create bucket";
        } finally {
            creatingBucket = false;
        }
    }

    async function handleDeleteBucket(id: string) {
        if (!auth.token) return;
        try {
            await deleteBucket(auth.token, id);
            buckets = buckets.filter((b) => b.id !== id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete bucket";
        }
    }

    async function openBucketDetail(bucket: Bucket) {
        if (!auth.token) return;
        activeBucket = bucket;
        bucketKeysError = null;
        createdKeySecret = null;
        newKeyName = "";
        bucketDetailDialog?.showModal();
        try {
            bucketKeys = await listBucketKeys(auth.token, bucket.id);
        } catch (e) {
            bucketKeysError = e instanceof Error ? e.message : "Failed to load access keys";
        }
    }

    async function handleCreateBucketKey() {
        if (!auth.token || !activeBucket || !newKeyName) return;
        creatingKey = true;
        bucketKeysError = null;
        try {
            const created = await createBucketKey(auth.token, activeBucket.id, { name: newKeyName });
            createdKeySecret = created.secret_access_key;
            newKeyName = "";
            bucketKeys = await listBucketKeys(auth.token, activeBucket.id);
        } catch (e) {
            bucketKeysError = e instanceof Error ? e.message : "Failed to create access key";
        } finally {
            creatingKey = false;
        }
    }

    function bucketEndpointUrl(bucket: Bucket): string {
        if (bucket.exposure === "external") {
            return `${window.location.origin}/api/s3/${bucket.global_alias}`;
        }
        return `http://s3.svc.${rgId}.internal:3900`;
    }

    function loadSettingsForm(workload: Workload) {
        settingsImage = workload.image;
        settingsEnvText = Object.entries(workload.env_vars ?? {})
            .map(([k, v]) => `${k}=${v}`)
            .join("\n");
        settingsRestartPolicy = workload.restart_policy as "always" | "on-failure" | "never";
        settingsMaxRestarts = workload.max_restarts !== null ? String(workload.max_restarts) : "";
        settingsError = null;
    }

    function openContainer(workload: Workload) {
        activeContainer = workload;
        containerActionError = null;
        containerDialog?.showModal();
        resolveNodeIp(workload.assigned_agent_id);
        loadSettingsForm(workload);
        if (workload.runtime_class === "vm") {
            containerDialogTab = "shell";
        } else {
            containerDialogTab = "logs";
            startLogsStream(workload);
        }
    }

    function closeContainer() {
        stopLogsStream();
        stopExecSession();
        containerDialog?.close();
        activeContainer = null;
    }

    function switchTab(tab: "logs" | "shell" | "insights" | "network" | "settings") {
        if (containerDialogTab === tab || !activeContainer) return;
        const isVm = activeContainer.runtime_class === "vm";

        if (containerDialogTab === "logs") stopLogsStream();
        if (containerDialogTab === "shell" && !isVm) stopExecSession();

        containerDialogTab = tab;

        if (tab === "logs") startLogsStream(activeContainer);
        if (tab === "shell" && !isVm) startExecSession(activeContainer);
    }

    async function startLogsStream(workload: Workload) {
        if (!auth.token) return;

        logsLines = [];
        logsError = null;
        logsAbort = new AbortController();
        const decoder = new TextDecoder();

        try {
            const body = await streamWorkloadLogs(auth.token, workload.id, logsAbort.signal);
            const reader = body.getReader();

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                const chunk = decoder.decode(value, { stream: true });
                if (chunk) {
                    logsLines = [...logsLines, ...chunk.split("\n").filter((l) => l.length > 0)];
                }
            }
        } catch (e) {
            if (e instanceof Error && e.name !== "AbortError") {
                logsError = e.message;
            }
        }
    }

    function stopLogsStream() {
        logsAbort?.abort();
        logsAbort = null;
    }

    async function startExecSession(workload: Workload) {
        if (!auth.token || workload.status !== "running") return;

        execError = null;
        await tick();

        if (!execTerminalEl) return;

        const { Terminal } = await import("@xterm/xterm");
        const { FitAddon } = await import("@xterm/addon-fit");

        execTerminal = new Terminal({ convertEol: true, cursorBlink: true });
        execFitAddon = new FitAddon();
        execTerminal.loadAddon(execFitAddon);
        execTerminal.open(execTerminalEl);
        execFitAddon.fit();

        try {
            execSocket = await openWorkloadExecSocket(auth.token, workload.id);
            execSocket.binaryType = "arraybuffer";

            execSocket.onmessage = (event) => {
                if (execTerminal) {
                    const data =
                        event.data instanceof ArrayBuffer
                            ? new Uint8Array(event.data)
                            : event.data;
                    execTerminal.write(data);
                }
            };
            execSocket.onerror = () => {
                execError = "Exec socket error";
            };
            execSocket.onclose = () => {
                execTerminal?.write("\r\n[session closed]\r\n");
            };

            execTerminal.onData((data) => {
                execSocket?.send(data);
            });
        } catch (e) {
            execError = e instanceof Error ? e.message : "Failed to start exec session";
        }
    }

    function stopExecSession() {
        execSocket?.close();
        execSocket = null;
        execTerminal?.dispose();
        execTerminal = null;
        execFitAddon = null;
    }

    async function handleStopContainer() {
        if (!auth.token || !activeContainer) return;
        containerActionBusy = true;
        containerActionError = null;
        try {
            await stopWorkload(auth.token, activeContainer.id);
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
            activeContainer = workloads.find((w) => w.id === activeContainer?.id) ?? null;
        } catch (e) {
            containerActionError = e instanceof Error ? e.message : "Failed to stop container";
        } finally {
            containerActionBusy = false;
        }
    }

    async function handleRestartContainer() {
        if (!auth.token || !activeContainer) return;
        containerActionBusy = true;
        containerActionError = null;
        try {
            await restartWorkload(auth.token, activeContainer.id);
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
            activeContainer = workloads.find((w) => w.id === activeContainer?.id) ?? null;
        } catch (e) {
            containerActionError = e instanceof Error ? e.message : "Failed to restart container";
        } finally {
            containerActionBusy = false;
        }
    }

    async function handleRedeployContainer() {
        if (!auth.token || !activeContainer) return;
        settingsSaving = true;
        settingsError = null;
        try {
            if (!settingsImage.trim()) throw new Error("Image cannot be empty");
            const env_vars: Record<string, string> = {};
            for (const line of settingsEnvText.split("\n")) {
                const trimmed = line.trim();
                if (!trimmed) continue;
                const idx = trimmed.indexOf("=");
                if (idx === -1) throw new Error(`Invalid env var line: "${trimmed}" (expected KEY=VALUE)`);
                env_vars[trimmed.slice(0, idx)] = trimmed.slice(idx + 1);
            }
            const max_restarts = settingsMaxRestarts.trim() === "" ? null : Number(settingsMaxRestarts);
            if (max_restarts !== null && (!Number.isInteger(max_restarts) || max_restarts < 0)) {
                throw new Error("Max restarts must be a non-negative integer");
            }

            await updateWorkload(auth.token, activeContainer.id, {
                image: settingsImage.trim(),
                env_vars,
                restart_policy: settingsRestartPolicy,
                max_restarts,
            });
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
            activeContainer = workloads.find((w) => w.id === activeContainer?.id) ?? null;
            if (activeContainer) loadSettingsForm(activeContainer);
        } catch (e) {
            settingsError = e instanceof Error ? e.message : "Failed to save settings";
        } finally {
            settingsSaving = false;
        }
    }

    async function handleDeleteContainer() {
        if (!auth.token || !activeContainer) return;
        containerActionBusy = true;
        containerActionError = null;
        try {
            await deleteWorkload(auth.token, activeContainer.id);
            workloads = workloads.filter((w) => w.id !== activeContainer?.id);
            closeContainer();
        } catch (e) {
            containerActionError = e instanceof Error ? e.message : "Failed to delete container";
        } finally {
            containerActionBusy = false;
        }
    }

    async function handleDeleteVolume(id: string) {
        if (!auth.token) return;
        try {
            await deleteVolume(auth.token, id);
            volumes = volumes.filter((v) => v.id !== id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete volume";
        }
    }

    async function handleRowRestart(id: string) {
        if (!auth.token) return;
        try {
            await restartWorkload(auth.token, id);
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to restart container";
        }
    }

    async function handleRowStop(id: string) {
        if (!auth.token) return;
        try {
            await stopWorkload(auth.token, id);
            workloads = await listResourceGroupWorkloads(auth.token, rgId);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to stop container";
        }
    }

    async function handleRowDelete(id: string) {
        if (!auth.token) return;
        try {
            await deleteWorkload(auth.token, id);
            workloads = workloads.filter((w) => w.id !== id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete container";
        }
    }

    async function downloadVpnConfig() {
        if (!auth.token) return;
        downloadingVpn = true;
        try {
            const resp = await fetch(`/api/resource-groups/${rgId}/vpn-config`, {
                headers: { Authorization: `Bearer ${auth.token}` },
            });
            if (!resp.ok) {
                const body = await resp.json().catch(() => ({}));
                throw new Error(body.error ?? `HTTP ${resp.status}`);
            }
            const blob = await resp.blob();
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            const cd = resp.headers.get("content-disposition") ?? "";
            const fn = cd.match(/filename="([^"]+)"/)?.[1] ?? `csfx-vpn.conf`;
            a.href = url;
            a.download = fn;
            a.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            error = e instanceof Error ? e.message : "VPN config download failed";
        } finally {
            downloadingVpn = false;
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

    type WorkloadStack = {
        stack_id: string;
        stack_name: string;
        children: Workload[];
    };

    type ResourceItem =
        | { kind: "container"; data: Workload }
        | { kind: "vm"; data: Workload }
        | { kind: "stack"; data: WorkloadStack }
        | { kind: "volume"; data: Volume }
        | { kind: "bucket"; data: Bucket };

    function groupWorkloadsByStack(items: Workload[]): ResourceItem[] {
        const standalone: Workload[] = [];
        const stacks = new Map<string, Workload[]>();

        for (const w of items) {
            if (!w.stack_id) {
                standalone.push(w);
                continue;
            }
            const group = stacks.get(w.stack_id) ?? [];
            group.push(w);
            stacks.set(w.stack_id, group);
        }

        const stackItems: ResourceItem[] = Array.from(stacks.entries()).map(
            ([stackId, children]): ResourceItem => ({
                kind: "stack",
                data: {
                    stack_id: stackId,
                    stack_name: `Stack ${stackId.slice(0, 8)}`,
                    children,
                },
            }),
        );

        return [
            ...standalone.map((w): ResourceItem => ({ kind: w.runtime_class === "vm" ? "vm" : "container", data: w })),
            ...stackItems,
        ];
    }

    let allResources = $derived<ResourceItem[]>([
        ...groupWorkloadsByStack(workloads),
        ...volumes.map((v): ResourceItem => ({ kind: "volume", data: v })),
        ...buckets.map((b): ResourceItem => ({ kind: "bucket", data: b })),
    ]);

    let filteredResources = $derived(
        allResources.filter((r) => {
            if (activeTab === "container" && r.kind !== "container" && r.kind !== "stack") return false;
            if (activeTab === "vm" && r.kind !== "vm") return false;
            if (activeTab === "volume" && r.kind !== "volume") return false;
            if (activeTab === "bucket" && r.kind !== "bucket") return false;
            if (!filterText) return true;
            const q = filterText.toLowerCase();
            if (r.kind === "container" || r.kind === "vm") {
                return r.data.name.toLowerCase().includes(q) || r.data.image.toLowerCase().includes(q);
            }
            if (r.kind === "stack") {
                return (
                    r.data.stack_name.toLowerCase().includes(q) ||
                    r.data.children.some((c) => c.name.toLowerCase().includes(q) || c.image.toLowerCase().includes(q))
                );
            }
            return r.data.name.toLowerCase().includes(q);
        })
    );

    function stackStatus(children: Workload[]): string {
        if (children.every((c) => c.status === "running")) return "running";
        if (children.some((c) => c.status === "failed" || c.status === "error")) return "failed";
        if (children.every((c) => c.status === "stopped")) return "stopped";
        return "pending";
    }

    function fmtBytes(bytes: number): string {
        if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
        if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(0)} MB`;
        return `${bytes} B`;
    }

    function openAppearanceDialog() {
        if (!group) return;
        editIcon = group.icon;
        editColor = group.color;
        appearanceError = null;
        appearanceDialog?.showModal();
    }

    async function handleSaveAppearance() {
        if (!auth.token || !group) return;
        savingAppearance = true;
        appearanceError = null;
        try {
            group = await updateResourceGroup(auth.token, group.id, { icon: editIcon, color: editColor });
            appearanceDialog?.close();
        } catch (e) {
            appearanceError = e instanceof Error ? e.message : "Failed to update appearance";
        } finally {
            savingAppearance = false;
        }
    }

    async function handleUploadIconImage(file: File) {
        if (!auth.token || !group) return;
        uploadingIconImage = true;
        iconImageError = null;
        try {
            group = await uploadResourceGroupIconImage(auth.token, group.id, file);
        } catch (e) {
            iconImageError = e instanceof Error ? e.message : "Failed to upload icon image";
        } finally {
            uploadingIconImage = false;
        }
    }

    async function handleRemoveIconImage() {
        if (!auth.token || !group) return;
        iconImageError = null;
        try {
            group = await deleteResourceGroupIconImage(auth.token, group.id);
        } catch (e) {
            iconImageError = e instanceof Error ? e.message : "Failed to remove icon image";
        }
    }

    async function handleTogglePin() {
        if (!auth.token || !group) return;
        try {
            group = await updateResourceGroup(auth.token, group.id, { pinned: !group.pinned });
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to update resource group";
        }
    }

    function openSettingsDialog() {
        if (!group) return;
        editName = group.name;
        editDescription = group.description ?? "";
        rgSettingsError = null;
        rgSettingsDialog?.showModal();
    }

    async function handleSaveSettings() {
        if (!auth.token || !group || !editName) return;
        savingRgSettings = true;
        rgSettingsError = null;
        try {
            group = await updateResourceGroup(auth.token, group.id, {
                name: editName,
                description: editDescription,
            });
            rgSettingsDialog?.close();
        } catch (e) {
            rgSettingsError = e instanceof Error ? e.message : "Failed to update resource group";
        } finally {
            savingRgSettings = false;
        }
    }

    async function handleDeleteGroup() {
        if (!auth.token || !group) return;
        try {
            await deleteResourceGroup(auth.token, group.id);
            goto("/resource-groups");
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete resource group";
        }
    }

    let loadStarted = false;

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            load();
        }
    });

    const hasTransientStatus = $derived(
        workloads.some((w) => isTransientStatus(w.status)) || volumes.some((v) => isTransientStatus(v.status))
    );

    let pollInterval: ReturnType<typeof setInterval> | null = null;

    $effect(() => {
        if (hasTransientStatus && !pollInterval) {
            pollInterval = setInterval(load, 2000);
        } else if (!hasTransientStatus && pollInterval) {
            clearInterval(pollInterval);
            pollInterval = null;
        }
    });

    onDestroy(() => {
        if (pollInterval) clearInterval(pollInterval);
    });

    $effect(() => {
        if (!activeContainer) return;
        const refreshed = workloads.find((w) => w.id === activeContainer?.id);
        if (refreshed && refreshed.status !== activeContainer.status) {
            activeContainer = refreshed;
        }
    });
</script>

<dialog
    bind:this={deployDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-lg rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => resetDeployForm()}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Deploy Container</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => deployDialog?.close()}
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
            <Button size="sm" variant="outline" onclick={() => deployDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleDeploy} disabled={deploying || !formName || !formImage}>
                {deploying ? "Deploying..." : "Deploy"}
            </Button>
        </div>
    </div>
</dialog>

<dialog
    bind:this={vmDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-lg rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => resetVmForm()}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Deploy Virtual Machine</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => vmDialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div class="flex flex-col gap-1 sm:col-span-2">
                <label class="text-xs text-muted-foreground" for="vm-name">Name</label>
                <input id="vm-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="my-vm" bind:value={vmFormName} />
            </div>
            <div class="flex flex-col gap-2 sm:col-span-2">
                <div class="flex items-center justify-between">
                    <span class="text-xs text-muted-foreground">ISO Image</span>
                    {#if vmExistingIsos.length > 0}
                        <div class="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-muted">
                            <button
                                type="button"
                                class="px-2 py-0.5 rounded text-xs font-medium transition-colors {vmIsoMode === 'existing' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                                onclick={() => (vmIsoMode = "existing")}
                            >
                                Existing
                            </button>
                            <button
                                type="button"
                                class="px-2 py-0.5 rounded text-xs font-medium transition-colors {vmIsoMode === 'upload' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                                onclick={() => (vmIsoMode = "upload")}
                            >
                                Upload new
                            </button>
                        </div>
                    {/if}
                </div>
                {#if vmExistingIsosLoading}
                    <p class="text-xs text-muted-foreground">Loading isos...</p>
                {:else if vmIsoMode === "existing"}
                    <select
                        id="vm-iso-existing"
                        class="border rounded px-3 py-1.5 text-sm bg-background"
                        bind:value={vmSelectedIsoKey}
                    >
                        {#each vmExistingIsos as obj (obj.key)}
                            <option value={obj.key}>{isoFileName(obj.key)}</option>
                        {/each}
                    </select>
                {:else}
                    <input
                        id="vm-iso"
                        type="file"
                        accept=".iso"
                        class="border rounded px-3 py-1.5 text-sm bg-background file:mr-3 file:rounded file:border-0 file:bg-muted file:px-2 file:py-1 file:text-xs"
                        onchange={handleVmIsoFileChange}
                    />
                    {#if vmDeploying && vmFormIsoFile}
                        <div class="h-1.5 rounded-full bg-muted overflow-hidden mt-1">
                            <div class="h-full bg-primary transition-all" style="width: {Math.round(vmUploadProgress * 100)}%"></div>
                        </div>
                    {/if}
                {/if}
            </div>
            {#if vmFormAdvanced}
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-cpu">CPU (millicores)</label>
                    <input id="vm-cpu" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2000" bind:value={vmFormCpuMillicores} />
                </div>
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-mem">Memory (MB)</label>
                    <input id="vm-mem" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2048" bind:value={vmFormMemoryMb} />
                </div>
                <div class="flex flex-col gap-1 sm:col-span-2">
                    <label class="text-xs text-muted-foreground" for="vm-disk">Disk (MB)</label>
                    <input id="vm-disk" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="20480" bind:value={vmFormDiskMb} />
                </div>
            {:else}
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-vcpus">vCPUs</label>
                    <input id="vm-vcpus" type="number" min="1" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2" bind:value={vmFormVcpus} />
                </div>
                <div class="flex flex-col gap-1">
                    <label class="text-xs text-muted-foreground" for="vm-mem-gb">Memory (GB)</label>
                    <input id="vm-mem-gb" type="number" min="1" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="2" bind:value={vmFormMemoryGb} />
                </div>
                <div class="flex flex-col gap-1 sm:col-span-2">
                    <label class="text-xs text-muted-foreground" for="vm-disk-gb">Disk (GB)</label>
                    <input id="vm-disk-gb" type="number" min="1" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="20" bind:value={vmFormDiskGb} />
                </div>
            {/if}
            <div class="sm:col-span-2">
                <button
                    type="button"
                    class="text-xs text-muted-foreground hover:text-foreground transition-colors underline underline-offset-2"
                    onclick={() => (vmFormAdvanced = !vmFormAdvanced)}
                >
                    {vmFormAdvanced ? "Use simple units" : "Advanced (millicores / MB)"}
                </button>
            </div>
        </div>
        <p class="text-xs text-muted-foreground">
            The ISO boots on first start. Open the console after deploying to complete the installation.
        </p>
        {#if vmError}
            <p class="text-xs text-destructive">{vmError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => vmDialog?.close()}>Cancel</Button>
            <Button
                size="sm"
                onclick={handleDeployVm}
                disabled={vmDeploying ||
                    !vmFormName ||
                    (vmIsoMode === "upload" ? !vmFormIsoFile : !vmSelectedIsoKey)}
            >
                {vmDeploying ? "Deploying..." : "Deploy"}
            </Button>
        </div>
    </div>
</dialog>

<dialog
    bind:this={resourcePickerDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-sm rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
>
    <div class="flex flex-col gap-4 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Add Resource</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => resourcePickerDialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="flex flex-col gap-2">
            {#each RESOURCE_TYPES as type}
                <button
                    class="flex items-center gap-3 rounded-lg border p-3 text-left hover:bg-muted/40 transition-colors"
                    onclick={() => openResourceType(type.key)}
                >
                    <div class="flex w-9 shrink-0 items-center justify-center">
                        <Icon icon={type.icon} width={20} height={20} />
                    </div>
                    <div>
                        <p class="text-sm font-medium leading-tight">{type.label}</p>
                        <p class="text-xs text-muted-foreground">{type.description}</p>
                    </div>
                </button>
            {/each}
        </div>
    </div>
</dialog>

<dialog
    bind:this={composeDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-6xl rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => resetComposeForm()}
>
    <div class="flex flex-col gap-4 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">{editingStackId ? "Edit Compose Stack" : "Deploy Docker Compose Stack"}</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => composeDialog?.close()}
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
            <Button size="sm" variant="outline" onclick={() => composeDialog?.close()}>Cancel</Button>
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

<dialog
    bind:this={appearanceDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-sm rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => { appearanceError = null; }}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Edit Appearance</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => appearanceDialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <IconPicker
            bind:icon={editIcon}
            bind:color={editColor}
            imageUrl={group?.has_icon_image ? `${resourceGroupIconImageUrl(group.id)}?t=${group.updated_at}` : null}
            uploadingImage={uploadingIconImage}
            imageError={iconImageError}
            onUploadImage={handleUploadIconImage}
            onRemoveImage={handleRemoveIconImage}
        />
        {#if appearanceError}
            <p class="text-xs text-destructive">{appearanceError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => appearanceDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleSaveAppearance} disabled={savingAppearance}>
                {savingAppearance ? "Saving..." : "Save"}
            </Button>
        </div>
    </div>
</dialog>

<dialog
    bind:this={rgSettingsDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-sm rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => { rgSettingsError = null; }}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Resource Group Settings</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => rgSettingsDialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="rg-edit-name">Name</label>
                <input id="rg-edit-name" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={editName} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="rg-edit-desc">Description</label>
                <input id="rg-edit-desc" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="Optional" bind:value={editDescription} />
            </div>
        </div>
        {#if rgSettingsError}
            <p class="text-xs text-destructive">{rgSettingsError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => rgSettingsDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleSaveSettings} disabled={savingRgSettings || !editName}>
                {savingRgSettings ? "Saving..." : "Save"}
            </Button>
        </div>
    </div>
</dialog>

<dialog
    bind:this={volumeDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-sm rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => { volumeError = null; }}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Create Volume</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => volumeDialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="v-name">Name</label>
                <input id="v-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="postgres-data" bind:value={volFormName} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="v-size">Size (GB)</label>
                <input id="v-size" type="number" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="10" bind:value={volFormSize} />
            </div>
        </div>
        {#if volumeError}
            <p class="text-xs text-destructive">{volumeError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => volumeDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleCreateVolume} disabled={creatingVolume || !volFormName}>
                {creatingVolume ? "Creating..." : "Create"}
            </Button>
        </div>
    </div>
</dialog>

<dialog
    bind:this={bucketDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-sm rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => { bucketError = null; }}
>
    <div class="flex flex-col gap-5 p-6">
        <div class="flex items-center justify-between">
            <h2 class="text-base font-semibold">Create Bucket</h2>
            <button
                class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                onclick={() => bucketDialog?.close()}
                aria-label="Close"
                title="Close"
            >
                <Icon icon="mdi:close" width={18} height={18} />
            </button>
        </div>
        <div class="grid grid-cols-1 gap-3">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="b-name">Name</label>
                <input id="b-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="assets" bind:value={bucketFormName} />
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-muted-foreground" for="b-exposure">Exposure</label>
                <select id="b-exposure" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={bucketFormExposure}>
                    <option value="internal">Internal (resource group only)</option>
                    <option value="external">External (public endpoint)</option>
                </select>
            </div>
        </div>
        {#if bucketError}
            <p class="text-xs text-destructive">{bucketError}</p>
        {/if}
        <div class="flex gap-2 justify-end">
            <Button size="sm" variant="outline" onclick={() => bucketDialog?.close()}>Cancel</Button>
            <Button size="sm" onclick={handleCreateBucket} disabled={creatingBucket || !bucketFormName}>
                {creatingBucket ? "Creating..." : "Create"}
            </Button>
        </div>
    </div>
</dialog>

<dialog
    bind:this={bucketDetailDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-lg rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => { activeBucket = null; createdKeySecret = null; }}
>
    {#if activeBucket}
        <div class="flex flex-col gap-5 p-6">
            <div class="flex items-center justify-between">
                <div>
                    <h2 class="text-base font-semibold">{activeBucket.name}</h2>
                    <p class="text-xs text-muted-foreground font-mono">{bucketEndpointUrl(activeBucket)}</p>
                </div>
                <button
                    class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    onclick={() => bucketDetailDialog?.close()}
                    aria-label="Close"
                    title="Close"
                >
                    <Icon icon="mdi:close" width={18} height={18} />
                </button>
            </div>

            <div class="flex flex-col gap-2">
                <p class="text-xs font-medium text-muted-foreground">Access Keys</p>
                {#if bucketKeys.length === 0}
                    <p class="text-xs text-muted-foreground">no access keys yet</p>
                {:else}
                    <div class="border rounded-lg divide-y">
                        {#each bucketKeys as key (key.id)}
                            <div class="flex items-center justify-between px-3 py-2 text-xs">
                                <div>
                                    <p class="font-medium">{key.name}</p>
                                    <p class="text-muted-foreground font-mono">{key.garage_key_id}</p>
                                </div>
                                <span class="text-muted-foreground">{key.permissions}</span>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>

            {#if createdKeySecret}
                <div class="border rounded-lg p-3 bg-muted/30 flex flex-col gap-1.5">
                    <p class="text-xs font-medium">Secret access key created</p>
                    <p class="text-xs font-mono break-all">{createdKeySecret}</p>
                    <p class="text-xs text-destructive">this will not be shown again, copy it now</p>
                    <Button
                        size="sm"
                        variant="outline"
                        class="self-start"
                        onclick={() => copyToClipboard(createdKeySecret ?? "", "bucket-secret")}
                    >
                        {copiedKey === "bucket-secret" ? "Copied" : "Copy"}
                    </Button>
                </div>
            {/if}

            {#if bucketKeysError}
                <p class="text-xs text-destructive">{bucketKeysError}</p>
            {/if}

            <div class="flex gap-2">
                <input
                    class="border rounded px-3 py-1.5 text-sm bg-background flex-1"
                    placeholder="key name"
                    bind:value={newKeyName}
                />
                <Button size="sm" onclick={handleCreateBucketKey} disabled={creatingKey || !newKeyName}>
                    {creatingKey ? "Creating..." : "Create Key"}
                </Button>
            </div>
        </div>
    {/if}
</dialog>

<dialog
    bind:this={containerDialog}
    class="fixed inset-0 z-50 m-auto w-full max-w-4xl h-[80vh] rounded-xl border bg-background shadow-xl p-0 backdrop:bg-black/40"
    onclose={() => closeContainer()}
>
    {#if activeContainer}
        <div class="flex flex-col h-full">
            <div class="flex items-center justify-between px-6 py-4 border-b shrink-0 gap-4">
                <div class="flex items-center gap-2.5 min-w-0">
                    <Icon icon={activeContainer.runtime_class === "vm" ? "mdi:monitor" : resolveImageIcon(activeContainer.image)} width={20} height={20} class="shrink-0" />
                    <div class="min-w-0">
                        <h2 class="text-base font-semibold leading-tight truncate">{activeContainer.service_name ?? activeContainer.name}</h2>
                        <p class="text-xs text-muted-foreground font-mono truncate">{activeContainer.image}</p>
                    </div>
                    <StatusBadge status={activeContainer.status} class="shrink-0" />
                </div>
                <div class="flex items-center gap-1 shrink-0">
                    <button
                        class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-40 disabled:pointer-events-none"
                        onclick={handleRestartContainer}
                        disabled={containerActionBusy}
                        aria-label="Restart"
                        title="Restart"
                    >
                        <Icon icon="mdi:restart" width={18} height={18} />
                    </button>
                    <button
                        class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-40 disabled:pointer-events-none"
                        onclick={handleStopContainer}
                        disabled={containerActionBusy || activeContainer.desired_state === "stopped"}
                        aria-label="Stop"
                        title="Stop"
                    >
                        <Icon icon="mdi:stop-circle-outline" width={18} height={18} />
                    </button>
                    <button
                        class="flex items-center justify-center w-8 h-8 rounded-full text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-40 disabled:pointer-events-none"
                        onclick={handleDeleteContainer}
                        disabled={containerActionBusy}
                        aria-label="Delete"
                        title="Delete"
                    >
                        <Icon icon="mdi:trash-can-outline" width={18} height={18} />
                    </button>
                    <div class="w-px h-5 bg-border mx-1"></div>
                    <button
                        class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                        onclick={() => containerDialog?.close()}
                        aria-label="Close"
                        title="Close"
                    >
                        <Icon icon="mdi:close" width={18} height={18} />
                    </button>
                </div>
            </div>
            {#if containerActionError}
                <p class="px-6 py-2 text-xs text-destructive shrink-0 border-b">{containerActionError}</p>
            {/if}
            <div class="px-6 py-2 border-b shrink-0">
                <div class="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-muted">
                    {#each [...(activeContainer.runtime_class === "vm" ? [] : [["logs", "Logs"]]), ["shell", activeContainer.runtime_class === "vm" ? "Console" : "Shell"], ["insights", "Performance"], ["network", "Network"], ...(activeContainer.stack_id ? [] : [["settings", "Settings"]])] as [tab, label]}
                        <button
                            class="px-3 py-1 rounded-md text-sm font-medium transition-all duration-200 {containerDialogTab === tab ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                            onclick={() => switchTab(tab as typeof containerDialogTab)}
                        >
                            {label}
                        </button>
                    {/each}
                </div>
            </div>
            <div class="flex-1 overflow-hidden">
                {#if containerDialogTab === "logs"}
                    <div class="h-full overflow-y-auto bg-black p-4 font-mono text-xs text-green-400">
                        {#if logsError}
                            <p class="text-red-400">{logsError}</p>
                        {/if}
                        {#each logsLines as line}
                            <p class="whitespace-pre-wrap">{line}</p>
                        {/each}
                    </div>
                {:else if containerDialogTab === "shell" && activeContainer.runtime_class === "vm"}
                    {#if activeContainer.status !== "running" || !auth.token}
                        <p class="p-4 text-xs text-muted-foreground">Console is only available while the vm is running.</p>
                    {:else}
                        {#key activeContainer.id}
                            <VncConsole token={auth.token} workloadId={activeContainer.id} />
                        {/key}
                    {/if}
                {:else if containerDialogTab === "shell"}
                    <div class="flex flex-col h-full">
                        {#if activeContainer.status !== "running"}
                            <p class="p-4 text-xs text-muted-foreground">Shell is only available while the container is running.</p>
                        {:else}
                            {#if execError}
                                <p class="px-4 py-2 text-xs text-destructive shrink-0">{execError}</p>
                            {/if}
                            <div class="flex-1 overflow-hidden bg-black p-2" bind:this={execTerminalEl}></div>
                        {/if}
                    </div>
                {:else if containerDialogTab === "insights"}
                    <div class="h-full overflow-y-auto p-6">
                        <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                            <div class="border rounded-lg p-4">
                                <p class="text-xs text-muted-foreground">CPU Usage</p>
                                <p class="text-2xl font-semibold mt-1">
                                    {activeContainer.cpu_usage_percent !== null ? `${activeContainer.cpu_usage_percent.toFixed(1)}%` : "-"}
                                </p>
                                <p class="text-xs text-muted-foreground mt-0.5">{activeContainer.cpu_millicores}m requested</p>
                            </div>
                            <div class="border rounded-lg p-4">
                                <p class="text-xs text-muted-foreground">Memory Usage</p>
                                <p class="text-2xl font-semibold mt-1">
                                    {activeContainer.memory_usage_bytes !== null ? fmtBytes(activeContainer.memory_usage_bytes) : "-"}
                                </p>
                                <p class="text-xs text-muted-foreground mt-0.5">{fmtBytes(activeContainer.memory_bytes)} requested</p>
                            </div>
                            <div class="border rounded-lg p-4">
                                <p class="text-xs text-muted-foreground">Network RX</p>
                                <p class="text-2xl font-semibold mt-1">
                                    {activeContainer.network_rx_bytes !== null ? fmtBytes(activeContainer.network_rx_bytes) : "-"}
                                </p>
                            </div>
                            <div class="border rounded-lg p-4">
                                <p class="text-xs text-muted-foreground">Network TX</p>
                                <p class="text-2xl font-semibold mt-1">
                                    {activeContainer.network_tx_bytes !== null ? fmtBytes(activeContainer.network_tx_bytes) : "-"}
                                </p>
                            </div>
                        </div>
                        <p class="text-xs text-muted-foreground mt-4">
                            {activeContainer.stats_updated_at ? `Last updated ${new Date(activeContainer.stats_updated_at).toLocaleTimeString()}` : "No stats reported yet."}
                        </p>
                        {#if activeContainer.restart_count > 0}
                            <p class="text-xs text-muted-foreground mt-1">
                                Restarted {activeContainer.restart_count} time{activeContainer.restart_count === 1 ? "" : "s"}{activeContainer.max_restarts !== null ? ` (max ${activeContainer.max_restarts})` : ""}
                            </p>
                        {/if}
                    </div>
                {:else if containerDialogTab === "network"}
                    <div class="h-full overflow-y-auto p-6 space-y-6">
                        {#if activeContainer.ports && activeContainer.ports.length > 0}
                            <div>
                                <p class="text-xs font-medium text-muted-foreground mb-2">Ports</p>
                                <div class="border rounded-lg divide-y">
                                    {#each activeContainer.ports as port}
                                        {@const rgHost = `${activeContainer.service_name ?? activeContainer.name}.svc.${rgId}.internal`}
                                        {@const rgAddress = `${rgHost}:${port.container_port}`}
                                        {@const rgPortAddress = port.rg_port ? `rg-gateway:${port.rg_port}` : null}
                                        {@const nodeIp = nodeIpCache[activeContainer.assigned_agent_id ?? ""]}
                                        {@const extAddress = port.node_port ? `${nodeIp ?? "node-ip"}:${port.node_port}` : null}
                                        <div class="p-3 space-y-2">
                                            <div class="flex items-center justify-between gap-3">
                                                <div class="min-w-0">
                                                    <p class="text-xs text-muted-foreground">RG-internal (DNS)</p>
                                                    <p class="font-mono text-sm truncate">{rgAddress}</p>
                                                </div>
                                                <button
                                                    class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors shrink-0"
                                                    onclick={() => copyToClipboard(rgAddress, `rg-${port.container_port}`)}
                                                    aria-label="Copy"
                                                    title="Copy"
                                                >
                                                    <Icon icon={copiedKey === `rg-${port.container_port}` ? "mdi:check" : "mdi:content-copy"} width={16} height={16} />
                                                </button>
                                            </div>
                                            {#if rgPortAddress}
                                                <div class="flex items-center justify-between gap-3">
                                                    <div class="min-w-0">
                                                        <p class="text-xs text-muted-foreground">RG port (mesh gateway)</p>
                                                        <p class="font-mono text-sm truncate">{rgPortAddress}</p>
                                                    </div>
                                                    <button
                                                        class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors shrink-0"
                                                        onclick={() => copyToClipboard(rgPortAddress, `rgp-${port.container_port}`)}
                                                        aria-label="Copy"
                                                        title="Copy"
                                                    >
                                                        <Icon icon={copiedKey === `rgp-${port.container_port}` ? "mdi:check" : "mdi:content-copy"} width={16} height={16} />
                                                    </button>
                                                </div>
                                            {/if}
                                            {#if extAddress}
                                                <div class="flex items-center justify-between gap-3">
                                                    <div class="min-w-0">
                                                        <p class="text-xs text-muted-foreground">External</p>
                                                        <p class="font-mono text-sm truncate">{extAddress}</p>
                                                    </div>
                                                    <button
                                                        class="flex items-center justify-center w-8 h-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors shrink-0"
                                                        onclick={() => copyToClipboard(extAddress, `ext-${port.container_port}`)}
                                                        aria-label="Copy"
                                                        title="Copy"
                                                    >
                                                        <Icon icon={copiedKey === `ext-${port.container_port}` ? "mdi:check" : "mdi:content-copy"} width={16} height={16} />
                                                    </button>
                                                </div>
                                            {/if}
                                        </div>
                                    {/each}
                                </div>
                            </div>
                            <p class="text-xs text-muted-foreground">
                                Reachable over VPN via the RG-internal address, or externally via the node IP if a node port is set. Use "Connect VPN" to resolve RG-internal hostnames.
                            </p>
                        {:else}
                            <p class="text-sm text-muted-foreground">No ports configured for this container.</p>
                        {/if}
                    </div>
                {:else if !activeContainer.stack_id}
                    <div class="h-full overflow-y-auto p-6 space-y-5">
                        <div class="flex flex-col gap-1">
                            <label class="text-xs text-muted-foreground" for="s-image">Image</label>
                            <input
                                id="s-image"
                                class="border rounded px-3 py-1.5 text-sm bg-background font-mono"
                                placeholder="nginx:latest"
                                bind:value={settingsImage}
                            />
                        </div>
                        <div class="flex flex-col gap-1">
                            <label class="text-xs text-muted-foreground" for="s-env">Environment variables</label>
                            <textarea
                                id="s-env"
                                class="border rounded px-3 py-2 text-sm bg-background font-mono h-40 resize-y"
                                placeholder="KEY=value"
                                bind:value={settingsEnvText}
                            ></textarea>
                            <p class="text-xs text-muted-foreground">One KEY=VALUE per line.</p>
                        </div>
                        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            <div class="flex flex-col gap-1">
                                <label class="text-xs text-muted-foreground" for="s-restart-policy">Restart policy</label>
                                <select id="s-restart-policy" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={settingsRestartPolicy}>
                                    <option value="always">Always</option>
                                    <option value="on-failure">On failure</option>
                                    <option value="never">Never</option>
                                </select>
                            </div>
                            <div class="flex flex-col gap-1">
                                <label class="text-xs text-muted-foreground" for="s-max-restarts">Max restarts</label>
                                <input
                                    id="s-max-restarts"
                                    type="number"
                                    min="0"
                                    class="border rounded px-3 py-1.5 text-sm bg-background"
                                    placeholder="Unlimited"
                                    bind:value={settingsMaxRestarts}
                                />
                            </div>
                        </div>
                        <p class="text-xs text-muted-foreground">
                            Redeploying applies the new configuration by recreating the container, pulling the image again if changed.
                        </p>
                        {#if settingsError}
                            <p class="text-xs text-destructive">{settingsError}</p>
                        {/if}
                        <div class="flex justify-end">
                            <Button size="sm" onclick={handleRedeployContainer} disabled={settingsSaving}>
                                {settingsSaving ? "Redeploying..." : "Redeploy"}
                            </Button>
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    {/if}
</dialog>

<div class="flex min-h-0 flex-1">
    <div class="hidden md:flex flex-col w-64 shrink-0 border-r border-border p-4">
        {#if group}
                <p class="text-lg font-light text-foreground/70 truncate">{group.name}</p>
                <div class="flex items-center gap-2 mt-3">
                    <button
                        class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md border text-xs font-medium shadow-sm hover:shadow transition-shadow"
                    >
                        <Icon icon="mdi:share-variant-outline" width={13} height={13} />
                        Share
                    </button>
                    <button
                        class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md bg-white text-black text-xs font-medium hover:bg-white/90 transition-colors"
                    >
                        <Icon icon="mdi:open-in-new" width={13} height={13} />
                        Visit
                    </button>
                </div>

                <div class="flex flex-col gap-3 mt-5 pb-5 border-b border-dashed border-border">
                    <div class="flex items-center justify-between gap-2">
                        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                            <Icon icon="mdi:pulse" width={14} height={14} />
                            Status
                        </span>
                        <StatusBadge status={group.status} />
                    </div>
                    <div class="flex items-center justify-between gap-2">
                        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                            <Icon icon="mdi:calendar-outline" width={14} height={14} />
                            Created
                        </span>
                        <span class="text-xs">{group.created_at.slice(0, 10)}</span>
                    </div>
                    <div class="flex items-center justify-between gap-2">
                        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                            <Icon icon="mdi:lan" width={14} height={14} />
                            Network
                        </span>
                        <span class="text-xs font-mono px-1.5 py-0.5 rounded-full border">{group.internal_cidr}</span>
                    </div>
                    <div class="flex items-center justify-between gap-2">
                        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                            <Icon icon="mdi:pin-outline" width={14} height={14} />
                            Pinned
                        </span>
                        <span class="text-xs">{group.pinned ? "Yes" : "No"}</span>
                    </div>
                </div>

                <div class="flex flex-col gap-3 mt-5">
                    <h3 class="text-xs font-semibold">Resources</h3>
                    {#each [
                        { label: "Containers", running: workloads.filter((w) => w.status === "running").length, total: workloads.length },
                        { label: "Volumes", running: volumes.filter((v) => v.status === "attached" || v.status === "in_use").length, total: volumes.length },
                        { label: "Buckets", running: buckets.filter((b) => b.status === "active").length, total: buckets.length },
                    ] as row}
                        <div class="flex flex-col gap-1">
                            <div class="flex items-center justify-between">
                                <span class="text-xs text-muted-foreground">{row.label}</span>
                                <span class="text-xs text-muted-foreground">{row.running}/{row.total}</span>
                            </div>
                            <div class="h-1.5 rounded-full bg-muted overflow-hidden">
                                <div
                                    class="h-full rounded-full bg-green-500"
                                    style="width: {row.total === 0 ? 0 : (row.running / row.total) * 100}%"
                                ></div>
                            </div>
                        </div>
                    {/each}
                </div>
        {/if}
    </div>

    <div class="flex min-w-0 flex-1 flex-col">
        <header class="flex h-16 shrink-0 items-center gap-3 px-4">
            <Button variant="ghost" size="icon-sm" onclick={() => goto("/resource-groups")} aria-label="Back to resource groups">
                <Icon icon="mdi:arrow-left" width={16} height={16} />
            </Button>
            <div class="flex-1 flex justify-center">
                <button
                    type="button"
                    onclick={() => commandPalette.show()}
                    class="relative w-full max-w-sm text-left"
                >
                    <SearchIcon class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                    <span class="flex items-center h-9 w-full rounded-md border border-input bg-background pl-8 pr-14 text-sm text-muted-foreground hover:bg-accent/50 transition-colors">
                        Search anything
                    </span>
                    <kbd class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 rounded border border-border bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                        &#8984;K
                    </kbd>
                </button>
            </div>
            <div class="w-9"></div>
        </header>

    <div class="flex min-w-0 flex-1 flex-col gap-6 p-6">
    {#if loading}
        <p class="text-sm text-muted-foreground">Loading...</p>
    {:else if error && !group}
        <p class="text-sm text-destructive">{error}</p>
    {:else if group}
        <div class="flex items-start gap-4">
            <button
                class="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg border transition-opacity hover:opacity-80 overflow-hidden"
                style="background-color: {group.color}20; color: {group.color};"
                onclick={openAppearanceDialog}
                aria-label="Edit appearance"
                title="Edit appearance"
            >
                <RgIcon id={group.id} icon={group.icon} color={group.color} hasIconImage={group.has_icon_image} size={24} />
            </button>
            <div class="flex-1 min-w-0">
                <div class="flex items-center gap-3 flex-wrap">
                    <h1 class="text-xl font-semibold tracking-tight">{group.name}</h1>
                    <StatusBadge status={group.status} />
                </div>
                {#if group.description}
                    <p class="text-sm text-muted-foreground mt-0.5">{group.description}</p>
                {/if}
                <p class="text-xs text-muted-foreground font-mono mt-1">{group.internal_cidr}</p>
            </div>
            <div class="flex items-center gap-2 shrink-0 flex-wrap justify-end">
                <button
                    class="flex items-center justify-center w-9 h-9 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    onclick={handleTogglePin}
                    aria-label={group.pinned ? "Unpin" : "Pin"}
                    title={group.pinned ? "Unpin" : "Pin"}
                >
                    <Icon
                        icon={group.pinned ? "mdi:pin" : "mdi:pin-outline"}
                        width={18}
                        height={18}
                        class="transition-transform duration-200 {group.pinned ? 'scale-110' : 'scale-100'}"
                    />
                </button>
                <button
                    class="flex items-center justify-center w-9 h-9 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    onclick={openSettingsDialog}
                    aria-label="Settings"
                    title="Settings"
                >
                    <Icon icon="mdi:cog-outline" width={18} height={18} />
                </button>
                <button
                    class="flex items-center justify-center w-9 h-9 rounded-full text-destructive hover:bg-destructive/10 transition-colors"
                    onclick={handleDeleteGroup}
                    aria-label="Delete"
                    title="Delete"
                >
                    <Icon icon="mdi:trash-can-outline" width={18} height={18} />
                </button>
                <Button size="sm" variant="outline" onclick={downloadVpnConfig} disabled={downloadingVpn}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
                    {downloadingVpn ? "Generating..." : "Connect VPN"}
                </Button>
                <Button size="sm" variant="outline" onclick={() => volumeDialog?.showModal()}>
                    Add Volume
                </Button>
                <Button size="sm" onclick={() => resourcePickerDialog?.showModal()}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                    Add Resource
                </Button>
            </div>
        </div>

        {#if error}
            <p class="text-xs text-destructive">{error}</p>
        {/if}

        <div class="border rounded-lg overflow-hidden">
            <div class="px-4 py-3 border-b flex items-center justify-between gap-4 flex-wrap">
                <div class="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-muted">
                    {#each [["all", `All ${allResources.length}`], ["container", `Container ${workloads.filter((w) => w.runtime_class !== "vm").length}`], ["vm", `VM ${workloads.filter((w) => w.runtime_class === "vm").length}`], ["volume", `Volume ${volumes.length}`], ["bucket", `Bucket ${buckets.length}`]] as [tab, label]}
                        <button
                            class="px-3 py-1 rounded-md text-sm font-medium transition-all duration-200 {activeTab === tab ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
                            onclick={() => (activeTab = tab as typeof activeTab)}
                        >
                            {label}
                        </button>
                    {/each}
                </div>
                <div class="flex items-center gap-2 border rounded px-3 py-1.5 text-sm min-w-[180px]">
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-muted-foreground shrink-0"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                    <input class="bg-transparent outline-none flex-1 text-sm" placeholder="Filter resources..." bind:value={filterText} />
                </div>
            </div>

            <table class="w-full text-sm">
                <thead>
                    <tr class="bg-muted/30 border-b">
                        <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Resource</th>
                        <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Kind</th>
                        <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Host / Size</th>
                        <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Load</th>
                        <th class="text-left px-4 py-2.5 font-medium text-muted-foreground text-xs">Status</th>
                        <th class="px-4 py-2.5"></th>
                    </tr>
                </thead>
                <tbody>
                    {#if filteredResources.length === 0}
                        <tr>
                            <td colspan="6" class="px-4 py-10 text-center text-muted-foreground text-sm">
                                {allResources.length === 0 ? "No resources yet. Deploy a container or create a volume." : "No resources match filter."}
                            </td>
                        </tr>
                    {:else}
                        {#snippet workloadRow(w: Workload, indented: boolean)}
                            <tr
                                class="border-t hover:bg-muted/20 transition-colors cursor-pointer"
                                onclick={() => w.runtime_class === "vm" ? goto(`/resource-groups/${rgId}/vm/${w.id}`) : openContainer(w)}
                            >
                                <td class="px-4 py-3">
                                    <div class="flex items-center gap-2.5" class:ps-6={indented}>
                                        <div class="flex w-5 shrink-0 items-center justify-center">
                                            <Icon icon={w.runtime_class === "vm" ? "mdi:monitor" : resolveImageIcon(w.image)} width={20} height={20} />
                                        </div>
                                        <p class="font-medium leading-tight">{w.service_name ?? w.name}</p>
                                    </div>
                                </td>
                                <td class="px-4 py-3">
                                    <span class="text-xs px-2 py-0.5 rounded border font-medium">{w.runtime_class === "vm" ? "VM" : "Container"}</span>
                                </td>
                                <td class="px-4 py-3 text-xs text-muted-foreground font-mono">
                                    {w.assigned_agent_id ? w.assigned_agent_id.slice(0, 8) : "-"}
                                </td>
                                <td class="px-4 py-3">
                                    <div class="flex items-center gap-2 min-w-[120px]">
                                        <span class="text-xs text-muted-foreground w-8">{w.cpu_millicores}m</span>
                                        <div class="flex-1 h-1 rounded-full bg-muted overflow-hidden">
                                            <div class="h-full rounded-full bg-foreground/60" style="width: {Math.min(100, w.cpu_millicores / 10)}%"></div>
                                        </div>
                                    </div>
                                </td>
                                <td class="px-4 py-3">
                                    <div class="flex items-center gap-1.5">
                                        <StatusBadge status={w.status} />
                                        {#if w.restart_count > 0}
                                            <span
                                                class="text-xs px-1.5 py-0.5 rounded-full font-medium bg-amber-500/10 text-amber-600"
                                                title="Restarted {w.restart_count} time{w.restart_count === 1 ? '' : 's'}{w.max_restarts !== null ? ` (max ${w.max_restarts})` : ''}"
                                            >
                                                ↻ {w.restart_count}
                                            </span>
                                        {/if}
                                    </div>
                                </td>
                                <td class="px-4 py-3">
                                    <div class="flex items-center justify-end gap-1">
                                        <button
                                            class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                                            onclick={(e) => { e.stopPropagation(); handleRowRestart(w.id); }}
                                            aria-label="Restart"
                                            title="Restart"
                                        >
                                            <Icon icon="mdi:restart" width={16} height={16} />
                                        </button>
                                        <button
                                            class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-40 disabled:pointer-events-none"
                                            onclick={(e) => { e.stopPropagation(); handleRowStop(w.id); }}
                                            disabled={w.desired_state === "stopped"}
                                            aria-label="Stop"
                                            title="Stop"
                                        >
                                            <Icon icon="mdi:stop-circle-outline" width={16} height={16} />
                                        </button>
                                        <button
                                            class="flex items-center justify-center w-7 h-7 rounded-full text-destructive hover:bg-destructive/10 transition-colors"
                                            onclick={(e) => { e.stopPropagation(); handleRowDelete(w.id); }}
                                            aria-label="Delete"
                                            title="Delete"
                                        >
                                            <Icon icon="mdi:trash-can-outline" width={16} height={16} />
                                        </button>
                                    </div>
                                </td>
                            </tr>
                        {/snippet}
                        {#each filteredResources as item (item.kind + (item.kind === "stack" ? item.data.stack_id : item.data.id))}
                            {#if item.kind === "container" || item.kind === "vm"}
                                {@render workloadRow(item.data, false)}
                            {:else if item.kind === "stack"}
                                {@const stack = item.data}
                                {@const expanded = expandedStacks.has(stack.stack_id)}
                                <tr
                                    class="border-t hover:bg-muted/20 transition-colors cursor-pointer"
                                    onclick={() => openStackEditor(stack.stack_id)}
                                >
                                    <td class="px-4 py-3">
                                        <div class="flex items-center gap-2.5">
                                            <button
                                                class="flex items-center justify-center w-5 h-5 shrink-0 rounded hover:bg-muted transition-colors"
                                                onclick={(e) => { e.stopPropagation(); toggleStack(stack.stack_id); }}
                                                aria-label={expanded ? "Collapse" : "Expand"}
                                                title={expanded ? "Collapse" : "Expand"}
                                            >
                                                <Icon icon={expanded ? "mdi:chevron-up" : "mdi:chevron-down"} width={16} height={16} class="text-muted-foreground" />
                                            </button>
                                            <div class="flex w-5 shrink-0 items-center justify-center">
                                                <Icon icon="logos:docker-icon" width={20} height={20} />
                                            </div>
                                            <div>
                                                <p class="font-medium leading-tight">{stack.stack_name}</p>
                                                <p class="text-xs text-muted-foreground">{stack.children.length} services</p>
                                            </div>
                                        </div>
                                    </td>
                                    <td class="px-4 py-3">
                                        <span class="text-xs px-2 py-0.5 rounded border font-medium">Compose Stack</span>
                                    </td>
                                    <td class="px-4 py-3 text-xs text-muted-foreground font-mono">
                                        {stack.children[0]?.assigned_agent_id ? stack.children[0].assigned_agent_id!.slice(0, 8) : "-"}
                                    </td>
                                    <td class="px-4 py-3 text-xs text-muted-foreground">-</td>
                                    <td class="px-4 py-3">
                                        <StatusBadge
                                            status={stackStatus(stack.children)}
                                            label="{stack.children.filter((c) => c.status === 'running').length}/{stack.children.length} running"
                                        />
                                    </td>
                                    <td class="px-4 py-3">
                                        <div class="flex items-center justify-end gap-1">
                                            <button
                                                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                                                onclick={(e) => { e.stopPropagation(); handleRestartStack(stack.stack_id); }}
                                                aria-label="Restart stack"
                                                title="Restart stack"
                                            >
                                                <Icon icon="mdi:restart" width={16} height={16} />
                                            </button>
                                            <button
                                                class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                                                onclick={(e) => { e.stopPropagation(); handleStopStack(stack.stack_id); }}
                                                aria-label="Stop stack"
                                                title="Stop stack"
                                            >
                                                <Icon icon="mdi:stop-circle-outline" width={16} height={16} />
                                            </button>
                                            <button
                                                class="flex items-center justify-center w-7 h-7 rounded-full text-destructive hover:bg-destructive/10 transition-colors"
                                                onclick={(e) => { e.stopPropagation(); handleDeleteStack(stack.stack_id); }}
                                                aria-label="Delete stack"
                                                title="Delete stack"
                                            >
                                                <Icon icon="mdi:trash-can-outline" width={16} height={16} />
                                            </button>
                                        </div>
                                    </td>
                                </tr>
                                {#if expanded}
                                    {#each stack.children as child (child.id)}
                                        {@render workloadRow(child, true)}
                                    {/each}
                                {/if}
                            {:else if item.kind === "volume"}
                                {@const v = item.data}
                                <tr class="border-t hover:bg-muted/20 transition-colors">
                                    <td class="px-4 py-3">
                                        <div class="flex items-center gap-2.5">
                                            <div class="flex h-7 w-7 shrink-0 items-center justify-center rounded border bg-muted/50">
                                                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
                                            </div>
                                            <div>
                                                <p class="font-medium leading-tight">{v.name}</p>
                                                <p class="text-xs text-muted-foreground">{v.size_gb} GB · {v.pool}</p>
                                            </div>
                                        </div>
                                    </td>
                                    <td class="px-4 py-3">
                                        <span class="text-xs px-2 py-0.5 rounded border font-medium">Volume</span>
                                    </td>
                                    <td class="px-4 py-3 text-xs text-muted-foreground">
                                        {v.size_gb} GB
                                    </td>
                                    <td class="px-4 py-3">
                                        <div class="flex items-center gap-2 min-w-[120px]">
                                            <span class="text-xs text-muted-foreground w-8">{v.size_gb}G</span>
                                            <div class="flex-1 h-1 rounded-full bg-muted overflow-hidden">
                                                <div class="h-full rounded-full bg-foreground/30" style="width: {Math.min(100, v.size_gb)}%"></div>
                                            </div>
                                        </div>
                                    </td>
                                    <td class="px-4 py-3">
                                        <StatusBadge status={v.status} />
                                    </td>
                                    <td class="px-4 py-3 text-right">
                                        <Button
                                            size="sm"
                                            variant="ghost"
                                            class="text-destructive hover:text-destructive h-7 px-2 text-xs"
                                            onclick={() => handleDeleteVolume(v.id)}
                                            disabled={v.status === "in_use"}
                                        >
                                            Delete
                                        </Button>
                                    </td>
                                </tr>
                            {:else if item.kind === "bucket"}
                                {@const b = item.data}
                                <tr
                                    class="border-t hover:bg-muted/20 transition-colors cursor-pointer"
                                    onclick={() => openBucketDetail(b)}
                                >
                                    <td class="px-4 py-3">
                                        <div class="flex items-center gap-2.5">
                                            <Icon icon="fluent-emoji-high-contrast:bucket" width={20} height={20} class="shrink-0" />
                                            <div>
                                                <p class="font-medium leading-tight">{b.name}</p>
                                                <p class="text-xs text-muted-foreground font-mono">{b.global_alias}</p>
                                            </div>
                                        </div>
                                    </td>
                                    <td class="px-4 py-3">
                                        <span class="text-xs px-2 py-0.5 rounded border font-medium">Bucket</span>
                                    </td>
                                    <td class="px-4 py-3 text-xs text-muted-foreground">
                                        {b.quota_max_size ? fmtBytes(b.quota_max_size) : "unlimited"}
                                    </td>
                                    <td class="px-4 py-3 text-xs text-muted-foreground">
                                        {b.exposure}
                                    </td>
                                    <td class="px-4 py-3">
                                        <StatusBadge status={b.status} />
                                    </td>
                                    <td class="px-4 py-3 text-right">
                                        <div class="flex items-center justify-end gap-1">
                                            <Button
                                                size="sm"
                                                variant="ghost"
                                                class="h-7 px-2 text-xs"
                                                onclick={(e) => { e.stopPropagation(); goto(`/buckets/${b.id}`); }}
                                            >
                                                Browse
                                            </Button>
                                            <Button
                                                size="sm"
                                                variant="ghost"
                                                class="text-destructive hover:text-destructive h-7 px-2 text-xs"
                                                onclick={(e) => { e.stopPropagation(); handleDeleteBucket(b.id); }}
                                            >
                                                Delete
                                            </Button>
                                        </div>
                                    </td>
                                </tr>
                            {/if}
                        {/each}
                    {/if}
                </tbody>
            </table>
        </div>
    {/if}
    </div>
    </div>
</div>
