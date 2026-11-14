import type { Tone } from "$lib/utils/dashboard";

const SUCCESS = new Set(["running", "active", "available", "in_use", "attached", "online", "healthy", "ready"]);
const TRANSIENT = new Set(["pending", "scheduled", "attaching", "detaching", "pulling", "creating", "starting", "deleting"]);
const DANGER = new Set(["failed", "error", "offline", "critical", "unhealthy"]);

export function statusTone(status: string): Tone | "" {
    const normalized = status.toLowerCase();
    if (SUCCESS.has(normalized)) return "success";
    if (DANGER.has(normalized)) return "danger";
    return ["stopped", "unknown", "inactive"].includes(normalized) ? "" : "warning";
}

export function toneBadgeClass(tone: Tone | ""): string {
    return tone ? `cx-badge cx-badge--${tone}` : "cx-badge";
}

export function badgeClass(status: string): string {
    return toneBadgeClass(statusTone(status));
}

export function isTransientStatus(status: string): boolean {
    return TRANSIENT.has(status.toLowerCase());
}

export function statusLabel(status: string): string {
    const text = status.replaceAll("_", " ");
    return text.charAt(0).toUpperCase() + text.slice(1).toLowerCase();
}
