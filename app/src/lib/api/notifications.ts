import { authedFetch } from './http';

const API_BASE = '/api/admin/settings/notifications';

export type NtfySeverity = 'info' | 'warning' | 'critical';

export interface NtfySettings {
    enabled: boolean;
    server_url: string;
    topic: string;
    token_set: boolean;
    min_severity: NtfySeverity;
}

export interface NtfySettingsUpdate {
    enabled: boolean;
    server_url: string;
    topic: string;
    token?: string;
    min_severity: NtfySeverity;
}

async function failure(res: Response, fallback: string): Promise<Error> {
    const err = await res.json().catch(() => ({ error: `${fallback}: ${res.status}` }));
    return new Error(err.error ?? `${fallback}: ${res.status}`);
}

export async function getNtfySettings(token: string): Promise<NtfySettings> {
    const res = await authedFetch(API_BASE, { headers: { Authorization: `Bearer ${token}` } });
    if (!res.ok) throw await failure(res, 'Failed to load notification settings');
    return res.json();
}

export async function updateNtfySettings(token: string, update: NtfySettingsUpdate): Promise<NtfySettings> {
    const res = await authedFetch(API_BASE, {
        method: 'PUT',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(update),
    });
    if (!res.ok) throw await failure(res, 'Failed to save notification settings');
    return res.json();
}

export async function sendTestNotification(token: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/test`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw await failure(res, 'Failed to send test notification');
}
