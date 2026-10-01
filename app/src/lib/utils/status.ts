import type { BadgeVariant } from "$lib/components/ui/badge/index.js";

const TRANSIENT_STATUSES = new Set([
    "pending",
    "scheduled",
    "attaching",
    "detaching",
    "pulling",
    "creating",
    "starting",
    "deleting",
]);

const VARIANT_BY_STATUS: Record<string, BadgeVariant> = {
    running: "success",
    active: "success",
    available: "success",
    in_use: "success",
    online: "success",
    failed: "destructive",
    error: "destructive",
    offline: "destructive",
    degraded: "warning",
    scheduled: "pending",
    attaching: "pending",
    detaching: "pending",
    pulling: "pending",
    creating: "warning",
    starting: "warning",
    pending: "warning",
    stopped: "outline",
    deleting: "outline",
};

const LABEL_BY_STATUS: Record<string, string> = {
    pulling: "Pulling image...",
    creating: "Creating container...",
    starting: "Starting...",
};

export function isTransientStatus(status: string): boolean {
    return TRANSIENT_STATUSES.has(status.toLowerCase());
}

export function statusVariant(status: string): BadgeVariant {
    return VARIANT_BY_STATUS[status.toLowerCase()] ?? "warning";
}

export function statusLabel(status: string): string {
    return LABEL_BY_STATUS[status.toLowerCase()] ?? status;
}
