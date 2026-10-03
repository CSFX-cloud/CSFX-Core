import { getContext, setContext } from "svelte";
import { auth } from "$lib/auth/store.svelte";
import {
    getResourceGroup,
    listResourceGroupBuckets,
    listResourceGroupVolumes,
    listResourceGroupWorkloads,
    type Bucket,
    type ResourceGroup,
    type Volume,
    type Workload,
} from "$lib/api/resource-groups";
import { isTransientStatus } from "$lib/utils/status.js";

const CONTEXT_KEY = Symbol("resource-group-state");

export class ResourceGroupState {
    group = $state<ResourceGroup | null>(null);
    workloads = $state<Workload[]>([]);
    volumes = $state<Volume[]>([]);
    buckets = $state<Bucket[]>([]);
    loading = $state(true);
    error = $state<string | null>(null);

    summaries = $derived([
        { label: "Containers", running: this.workloads.filter((w) => w.status === "running").length, total: this.workloads.length },
        { label: "Volumes", running: this.volumes.filter((v) => v.status === "attached" || v.status === "in_use").length, total: this.volumes.length },
        { label: "Buckets", running: this.buckets.filter((b) => b.status === "active").length, total: this.buckets.length },
    ]);

    hasTransientStatus = $derived(
        this.workloads.some((w) => isTransientStatus(w.status)) || this.volumes.some((v) => isTransientStatus(v.status)),
    );

    constructor(readonly rgId: string) {}

    load = async () => {
        if (!auth.token) return;
        try {
            [this.group, this.workloads, this.volumes, this.buckets] = await Promise.all([
                getResourceGroup(auth.token, this.rgId),
                listResourceGroupWorkloads(auth.token, this.rgId),
                listResourceGroupVolumes(auth.token, this.rgId),
                listResourceGroupBuckets(auth.token, this.rgId),
            ]);
        } catch (e) {
            this.error = e instanceof Error ? e.message : "Failed to load";
        } finally {
            this.loading = false;
        }
    };
}

export function provideResourceGroupState(state: ResourceGroupState) {
    setContext(CONTEXT_KEY, state);
}

export function useResourceGroupState(): ResourceGroupState {
    return getContext<ResourceGroupState>(CONTEXT_KEY);
}
