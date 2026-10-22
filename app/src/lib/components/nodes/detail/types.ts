export interface NodeActions {
    startMaintenance(): void;
    endMaintenance(): void;
    reboot(): void;
    cordon(): void;
    uncordon(): void;
    drain(): void;
    remove(): void;
    powerOff(): void;
}

export type NodeTab = "summary" | "hardware" | "storage" | "workloads" | "network" | "alerts" | "tasks";

export const NODE_TABS: { id: NodeTab; label: string }[] = [
    { id: "summary", label: "Summary" },
    { id: "hardware", label: "Hardware" },
    { id: "storage", label: "Storage" },
    { id: "workloads", label: "Workloads" },
    { id: "network", label: "Network" },
    { id: "alerts", label: "Alerts" },
    { id: "tasks", label: "Tasks" },
];
