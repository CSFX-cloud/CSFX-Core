import type { Tone } from "$lib/utils/dashboard";
import { statusTone } from "$lib/utils/tone";

export interface GroupHealth {
    label: string;
    tone: Tone | "";
}

export function groupHealth(groupStatus: string, resourceStatuses: string[]): GroupHealth {
    if (groupStatus === "deleting") return { label: "Deleting", tone: "warning" };
    if (resourceStatuses.length === 0) return { label: "Empty", tone: "" };
    const tones = resourceStatuses.map(statusTone);
    const count = (tone: Tone | "") => tones.filter((entry) => entry === tone).length;
    if (count("danger") > 0) return { label: `${count("danger")} failing`, tone: "danger" };
    if (count("warning") > 0) return { label: `${count("warning")} pending`, tone: "warning" };
    if (count("") > 0) return { label: `${count("success")}/${tones.length} running`, tone: "" };
    return { label: "Healthy", tone: "success" };
}
