import { api } from './http';

const SETTINGS_PATH = '/admin/settings/notifications';

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

export const getNtfySettings = () => api<NtfySettings>('GET', SETTINGS_PATH);

export const updateNtfySettings = (update: NtfySettingsUpdate) => api<NtfySettings>('PUT', SETTINGS_PATH, update);

export const sendTestNotification = () => api<void>('POST', `${SETTINGS_PATH}/test`);
