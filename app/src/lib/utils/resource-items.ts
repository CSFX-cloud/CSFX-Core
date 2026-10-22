import type { Gateway } from "$lib/api/ingress";
import type { Bucket, Volume, Workload } from "$lib/api/resource-groups";

export type ResourceTab = "all" | "compute" | "containers" | "storage";

export interface WorkloadStack {
    stack_id: string;
    stack_name: string;
    children: Workload[];
}

export type ResourceItem =
    | { kind: "container"; data: Workload }
    | { kind: "vm"; data: Workload }
    | { kind: "stack"; data: WorkloadStack }
    | { kind: "volume"; data: Volume }
    | { kind: "bucket"; data: Bucket };

export function groupWorkloadsByStack(items: Workload[]): ResourceItem[] {
    const standalone: Workload[] = [];
    const stacks = new Map<string, Workload[]>();

    for (const workload of items) {
        if (!workload.stack_id) {
            standalone.push(workload);
            continue;
        }
        const children = stacks.get(workload.stack_id) ?? [];
        children.push(workload);
        stacks.set(workload.stack_id, children);
    }

    const stackItems = Array.from(stacks.entries()).map(
        ([stackId, children]): ResourceItem => ({
            kind: "stack",
            data: { stack_id: stackId, stack_name: `Stack ${stackId.slice(0, 8)}`, children },
        }),
    );

    return [
        ...standalone.map((w): ResourceItem => ({ kind: w.runtime_class === "vm" ? "vm" : "container", data: w })),
        ...stackItems,
    ];
}

export function buildResourceItems(workloads: Workload[], volumes: Volume[], buckets: Bucket[]): ResourceItem[] {
    return [
        ...groupWorkloadsByStack(workloads),
        ...volumes.map((data): ResourceItem => ({ kind: "volume", data })),
        ...buckets.map((data): ResourceItem => ({ kind: "bucket", data })),
    ];
}

function matchesTab(item: ResourceItem, tab: ResourceTab): boolean {
    switch (tab) {
        case "all":
            return true;
        case "compute":
            return item.kind === "vm";
        case "containers":
            return item.kind === "container" || item.kind === "stack";
        case "storage":
            return item.kind === "volume" || item.kind === "bucket";
    }
}

export function filterResources(items: ResourceItem[], tab: ResourceTab): ResourceItem[] {
    return items.filter((item) => matchesTab(item, tab));
}

export function stackStatus(children: Workload[]): string {
    if (children.every((c) => c.status === "running")) return "running";
    if (children.some((c) => c.status === "failed" || c.status === "error")) return "failed";
    if (children.every((c) => c.status === "stopped")) return "stopped";
    return "pending";
}

export function workloadHref(base: string, workload: Workload, gateways: Gateway[]): string {
    const gateway = gateways.find((candidate) => candidate.workload_id === workload.id);
    if (gateway) return `${base}/gateway/${gateway.id}`;
    return `${base}/${workload.runtime_class === "vm" ? "vm" : "container"}/${workload.id}`;
}
