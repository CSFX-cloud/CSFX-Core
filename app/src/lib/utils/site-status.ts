import type { BuildPhases, DeploymentStatus } from "$lib/api/sites";
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

const PHASE_LABELS: [keyof BuildPhases, string][] = [
    ["clone_ms", "clone"],
    ["install_ms", "install"],
    ["build_ms", "build"],
    ["image_ms", "image"],
    ["upload_ms", "upload"],
];

export function phaseDuration(milliseconds: number): string {
    return milliseconds < MS_PER_SECOND ? `${milliseconds} ms` : buildDuration(milliseconds);
}

export function phaseSummary(phases: BuildPhases | null): string {
    if (!phases) return "";
    return PHASE_LABELS.filter(([key]) => phases[key] !== null && phases[key] !== undefined)
        .map(([key, label]) => `${label} ${phaseDuration(phases[key] as number)}`)
        .join(" · ");
}

export function shortSha(sha: string): string {
    return sha.slice(0, SHORT_SHA_LENGTH);
}

export function repoLabel(url: string): string {
    return url.replace(/^https?:\/\//, "").replace(/\.git$/, "");
}
