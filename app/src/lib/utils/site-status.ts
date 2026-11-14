import type { DeploymentStatus } from "$lib/api/sites";
import type { Tone } from "$lib/utils/dashboard";

type DeploymentView = { label: string; tone: Tone | "" };


const BY_STATUS: Record<DeploymentStatus, DeploymentView> = {
    ready: { label: "Ready", tone: "success" },
    queued: { label: "Queued", tone: "warning" },
    building: { label: "Building", tone: "warning" },
    failed: { label: "Failed", tone: "danger" },
    rolled_back: { label: "Rolled back", tone: "" },
};

const MS_PER_SECOND = 1000;
const SECONDS_PER_MINUTE = 60;
const SHORT_SHA_LENGTH = 7;

export function deploymentStatus(status: DeploymentStatus | undefined): DeploymentView {
    return status ? BY_STATUS[status] : { label: "No deployment", tone: "" };
}

export function buildDuration(milliseconds: number | null): string {
    if (milliseconds === null) return "—";
    const seconds = Math.round(milliseconds / MS_PER_SECOND);
    if (seconds < SECONDS_PER_MINUTE) return `${seconds} s`;
    return `${Math.floor(seconds / SECONDS_PER_MINUTE)} min ${seconds % SECONDS_PER_MINUTE} s`;
}

export function shortSha(sha: string): string {
    return sha.slice(0, SHORT_SHA_LENGTH);
}

export function repoLabel(url: string): string {
    return url.replace(/^https?:\/\//, "").replace(/\.git$/, "");
}
