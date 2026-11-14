import { api, ApiError } from './http';

export interface UpdateStatus {
    current_version: string;
    desired_version: string | null;
    build_status: string | null;
    last_result: string | null;
    paused: boolean;
}

export const getUpdateStatus = () =>
    api<UpdateStatus>('GET', '/system/update/status');

export const triggerUpdate = (version: string) =>
    api<void>('POST', '/system/update', { version });

export const pauseUpdate = () =>
    api<void>('POST', '/system/update/pause');

export const resumeUpdate = () =>
    api<void>('POST', '/system/update/resume');

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

export const getReleases = (includePre = false) =>
    api<ReleasesResponse>('GET', `/system/releases${includePre ? '?include_pre=true' : ''}`);

export async function getReleaseNotes(version: string): Promise<ReleaseNotes | null> {
    try {
        return await api<ReleaseNotes>('GET', `/system/releases/${encodeURIComponent(version)}/notes`);
    } catch (e) {
        if (e instanceof ApiError && e.status === 404) return null;
        throw e;
    }
}
