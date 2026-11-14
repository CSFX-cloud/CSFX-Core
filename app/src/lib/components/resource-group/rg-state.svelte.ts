import { getContext, setContext } from "svelte";
import { auth } from "$lib/auth/store.svelte";
import type { Gateway } from "$lib/api/ingress";
import { getResourceGroup, type Bucket, type ResourceGroup, type Volume, type Workload } from "$lib/api/resource-groups";
import type { Registry } from "$lib/api/registries";
import type { SiteSummary } from "$lib/api/sites";
import type { Vault } from "$lib/api/vaults";
import { loadGroupInventory } from "$lib/utils/delete-preview";
import { groupHealth } from "$lib/utils/group-health";
import { isTransientStatus } from "$lib/utils/tone";

const CONTEXT_KEY = Symbol("resource-group-state");

export class ResourceGroupState {
    group = $state<ResourceGroup | null>(null);
    workloads = $state<Workload[]>([]);
    volumes = $state<Volume[]>([]);
    buckets = $state<Bucket[]>([]);
    gateways = $state<Gateway[]>([]);
    vaults = $state<Vault[]>([]);
    sites = $state<SiteSummary[]>([]);
    registries = $state<Registry[]>([]);
    loading = $state(true);
    error = $state<string | null>(null);

    summaries = $derived([
        { label: "Containers", running: this.workloads.filter((w) => w.status === "running").length, total: this.workloads.length },
        { label: "Volumes", running: this.volumes.filter((v) => v.status === "attached" || v.status === "in_use").length, total: this.volumes.length },
        { label: "Buckets", running: this.buckets.filter((b) => b.status === "active").length, total: this.buckets.length },
    ]);

    health = $derived(
        groupHealth(this.group?.status ?? "", [...this.workloads, ...this.volumes].map((resource) => resource.status)),
    );

    hasTransientStatus = $derived(
        this.workloads.some((w) => isTransientStatus(w.status)) ||
            this.volumes.some((v) => isTransientStatus(v.status)) ||
            this.sites.some((site) => site.latest?.status === "queued" || site.latest?.status === "building"),
    );

    constructor(readonly rgId: string) {}

    load = async () => {
        if (!auth.token) return;
        try {
            const [group, inventory] = await Promise.all([getResourceGroup(auth.token, this.rgId), loadGroupInventory(auth.token, this.rgId)]);
            this.group = group;
            ({
                workloads: this.workloads,
                volumes: this.volumes,
                buckets: this.buckets,
                gateways: this.gateways,
                vaults: this.vaults,
                sites: this.sites,
                registries: this.registries,
            } = inventory);
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
