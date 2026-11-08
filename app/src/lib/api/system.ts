import { authedFetch } from './http';

const API_BASE = '/api';

export interface UpdateStatus {
    current_version: string;
    desired_version: string | null;
    build_status: string | null;
    last_result: string | null;
    paused: boolean;
}

export async function getUpdateStatus(token: string): Promise<UpdateStatus> {
    const res = await authedFetch(`${API_BASE}/system/update/status`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`update status fetch failed: ${res.status}`);
    return res.json();
}

export async function triggerUpdate(token: string, version: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/system/update`, {
        method: 'POST',
        headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
        },
        body: JSON.stringify({ version }),
    });
    if (!res.ok) throw new Error(`trigger update failed: ${res.status}`);
}

export async function pauseUpdate(token: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/system/update/pause`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`pause update failed: ${res.status}`);
}

export async function resumeUpdate(token: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/system/update/resume`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`resume update failed: ${res.status}`);
}

export interface ReleaseEntry {
    version: string;
    tag: string;
    prerelease: boolean;
    html_url: string;
    name: string | null;
    is_current: boolean;
    is_newer: boolean;
}

export interface ReleasesResponse {
    current_version: string;
    update_available: boolean;
    latest_update: string | null;
    releases: ReleaseEntry[];
}

export type NoteCategory = "new" | "improved" | "fixed" | "security";

export type NoteBlock =
    | { type: "heading"; text: string }
    | { type: "paragraph"; text: string }
    | { type: "list"; items: string[] }
    | { type: "image"; src: string; alt: string };

export interface ReleaseNotes {
    version: string;
    channel: "pre" | "stable";
    date: string;
    breaking: string[];
    sections: Partial<Record<NoteCategory, NoteBlock[]>>;
}

export async function getReleases(token: string, includePre = false): Promise<ReleasesResponse> {
    const url = `${API_BASE}/system/releases${includePre ? '?include_pre=true' : ''}`;
    const res = await authedFetch(url, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`releases fetch failed: ${res.status}`);
    return res.json();
}
