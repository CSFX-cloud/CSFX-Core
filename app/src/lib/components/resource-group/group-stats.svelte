<script lang="ts">
    import type { Bucket, Volume, Workload } from "$lib/api/resource-groups";
    import { formatCapacity, pluralize } from "$lib/utils/format";
    import { isTransientStatus } from "$lib/utils/status.js";

    const MILLICORES_PER_CORE = 1000;
    const BYTES_PER_GB = 1_073_741_824;
    const ADDRESS_BITS = 32;
    const NETWORK_AND_BROADCAST = 2;

    type Props = { workloads: Workload[]; volumes: Volume[]; buckets: Bucket[]; cidr: string };

    let { workloads, volumes, buckets, cidr }: Props = $props();

    const total = $derived(workloads.length + volumes.length + buckets.length);
    const pending = $derived([...workloads, ...volumes].filter((resource) => isTransientStatus(resource.status)).length);
    const vms = $derived(workloads.filter((workload) => workload.runtime_class === "vm").length);
    const vcpu = $derived(workloads.reduce((sum, workload) => sum + workload.cpu_millicores, 0) / MILLICORES_PER_CORE);
    const memory = $derived(workloads.reduce((sum, workload) => sum + workload.memory_bytes, 0));
    const blockBytes = $derived(volumes.reduce((sum, volume) => sum + volume.size_gb, 0) * BYTES_PER_GB);
    const addresses = $derived(addressCount(cidr));

    function addressCount(value: string): number | null {
        const prefix = Number.parseInt(value.split("/")[1] ?? "", 10);
        if (!Number.isInteger(prefix) || prefix > ADDRESS_BITS - 2) return null;
        return 2 ** (ADDRESS_BITS - prefix) - NETWORK_AND_BROADCAST;
    }
</script>

<section class="db-stats" aria-label="Summary">
    <div class="cx-stat">
        <p class="cx-stat__label">Resources</p>
        <p class="cx-stat__value stat">{total}</p>
        <div class="cx-stat__foot">
            {#if pending > 0}<span class="cx-badge cx-badge--warning">{pending} pending</span>{:else}{pluralize(volumes.length, "volume")} &middot; {pluralize(buckets.length, "bucket")}{/if}
        </div>
    </div>
    <div class="cx-stat">
        <p class="cx-stat__label">Compute</p>
        <p class="db-stat-value">{vcpu} <small>vCPU</small> &middot; {formatCapacity(memory)}</p>
        <div class="cx-stat__foot">{pluralize(vms, "VM")} &middot; {pluralize(workloads.length - vms, "container")}</div>
    </div>
    <div class="cx-stat">
        <p class="cx-stat__label">Storage</p>
        <p class="db-stat-value">{formatCapacity(blockBytes)}</p>
        <div class="cx-stat__foot">{volumes.length} block &middot; {buckets.length} object</div>
    </div>
    <div class="cx-stat">
        <p class="cx-stat__label">Private network</p>
        <p class="db-stat-value cx-mono" style="font-size: 18px">{cidr}</p>
        <div class="cx-stat__foot">{addresses === null ? "Unknown size" : `${addresses} addresses`}</div>
    </div>
</section>
