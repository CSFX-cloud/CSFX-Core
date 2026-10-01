import type { Node } from "$lib/api/nodes";
import type { Bucket, Volume, Workload } from "$lib/api/resource-groups";

export interface Segment {
    label: string;
    value: number;
    color: string;
}

const COLORS = {
    healthy: "#22c55e",
    problem: "#eab308",
    offline: "#ef4444",
    vm: "#6366f1",
    container: "#38bdf8",
    volume: "#a855f7",
    bucket: "#f59e0b",
};

export function summarizeNodes(nodes: Node[]): Segment[] {
    const count = (status: string) => nodes.filter((n) => n.status.toLowerCase() === status).length;
    const online = count("online");
    const offline = count("offline");
    return [
        { label: "Online", value: online, color: COLORS.healthy },
        { label: "Problems", value: nodes.length - online - offline, color: COLORS.problem },
        { label: "Offline", value: offline, color: COLORS.offline },
    ];
}

export function summarizeServices(workloads: Workload[], volumes: Volume[], buckets: Bucket[]): Segment[] {
    const vms = workloads.filter((w) => w.runtime_class === "vm").length;
    return [
        { label: "VMs", value: vms, color: COLORS.vm },
        { label: "Containers", value: workloads.length - vms, color: COLORS.container },
        { label: "Volumes", value: volumes.length, color: COLORS.volume },
        { label: "Buckets", value: buckets.length, color: COLORS.bucket },
    ];
}
