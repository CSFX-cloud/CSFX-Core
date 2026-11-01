import { authedFetch } from './http';

const API_BASE = '/api';

export interface Vault {
    id: string;
    resource_group_id: string;
    name: string;
    sealed: boolean;
    soft_delete_days: number;
    created_at: string;
}

export interface VaultOverview extends Vault {
    secret_count: number;
    grant_count: number;
    reads_24h: number;
    denied_24h: number;
}

export interface SecretSummary {
    name: string;
    version: number;
    updated_at: string;
    expires_at: string | null;
    deleted: boolean;
}

export interface SecretInput {
    name?: string;
    value: string;
    expires_at: string | null;
}

export interface RevealInput {
    reason: string;
    code: string;
    version?: number;
}

export interface RevealedSecret {
    name: string;
    version: number;
    value: string;
}

export type Capability = 'read_secrets' | 'reveal_secrets' | 'manage';

export interface VaultGrant {
    id: string;
    vault_id: string;
    principal: string;
    capability: Capability;
    scope: string;
    requires_approval: boolean;
    created_at: string;
}

export interface GrantInput {
    principal: string;
    capability: Capability;
    scope: string;
    requires_approval: boolean;
}

export interface VaultAuditEntry {
    id: string;
    occurred_at: string;
    actor: string;
    action: string;
    target: string;
    source: string;
    result: string;
}

function vaultsPath(rgId: string, vaultId?: string): string {
    const root = `${API_BASE}/resource-groups/${rgId}/vaults`;
    return vaultId ? `${root}/${vaultId}` : root;
}

function headers(token: string): HeadersInit {
    return { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' };
}

async function failure(res: Response, fallback: string): Promise<Error> {
    const body = await res.json().catch(() => ({}));
    return new Error(body.error ?? `${fallback}: ${res.status}`);
}

async function send<T>(token: string, url: string, method: string, fallback: string, body?: unknown): Promise<T> {
    const res = await authedFetch(url, {
        method,
        headers: headers(token),
        body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) throw await failure(res, fallback);
    return res.status === 204 ? (undefined as T) : res.json();
}

export const listVaults = (token: string, rgId: string) =>
    send<Vault[]>(token, vaultsPath(rgId), 'GET', 'Failed to list vaults');

export const createVault = (token: string, rgId: string, name: string) =>
    send<Vault>(token, vaultsPath(rgId), 'POST', 'Failed to create vault', { name });

export const getVault = (token: string, rgId: string, vaultId: string) =>
    send<VaultOverview>(token, vaultsPath(rgId, vaultId), 'GET', 'Failed to load vault');

export const deleteVault = (token: string, rgId: string, vaultId: string) =>
    send<void>(token, vaultsPath(rgId, vaultId), 'DELETE', 'Failed to delete vault');

export const setVaultSealed = (token: string, rgId: string, vaultId: string, sealed: boolean) =>
    send<Vault>(token, `${vaultsPath(rgId, vaultId)}/${sealed ? 'seal' : 'unseal'}`, 'POST', 'Failed to change vault state');

export const listSecrets = (token: string, rgId: string, vaultId: string) =>
    send<SecretSummary[]>(token, `${vaultsPath(rgId, vaultId)}/secrets`, 'GET', 'Failed to list secrets');

export const createSecret = (token: string, rgId: string, vaultId: string, input: SecretInput) =>
    send<{ name: string; version: number }>(token, `${vaultsPath(rgId, vaultId)}/secrets`, 'POST', 'Failed to create secret', input);

export const updateSecret = (token: string, rgId: string, vaultId: string, name: string, input: SecretInput) =>
    send<{ name: string; version: number }>(token, `${vaultsPath(rgId, vaultId)}/secrets/${name}`, 'PUT', 'Failed to update secret', input);

export const deleteSecret = (token: string, rgId: string, vaultId: string, name: string) =>
    send<void>(token, `${vaultsPath(rgId, vaultId)}/secrets/${name}`, 'DELETE', 'Failed to delete secret');

export const restoreSecret = (token: string, rgId: string, vaultId: string, name: string, version: number) =>
    send<void>(token, `${vaultsPath(rgId, vaultId)}/secrets/${name}/restore`, 'POST', 'Failed to restore secret', { version });

export const revealSecret = (token: string, rgId: string, vaultId: string, name: string, input: RevealInput) =>
    send<RevealedSecret>(token, `${vaultsPath(rgId, vaultId)}/secrets/${name}/reveal`, 'POST', 'Failed to reveal secret', input);

export const listGrants = (token: string, rgId: string, vaultId: string) =>
    send<VaultGrant[]>(token, `${vaultsPath(rgId, vaultId)}/grants`, 'GET', 'Failed to list access grants');

export const createGrant = (token: string, rgId: string, vaultId: string, input: GrantInput) =>
    send<VaultGrant>(token, `${vaultsPath(rgId, vaultId)}/grants`, 'POST', 'Failed to grant access', input);

export const deleteGrant = (token: string, rgId: string, vaultId: string, grantId: string) =>
    send<void>(token, `${vaultsPath(rgId, vaultId)}/grants/${grantId}`, 'DELETE', 'Failed to revoke access');

export const listVaultAudit = (token: string, rgId: string, vaultId: string) =>
    send<VaultAuditEntry[]>(token, `${vaultsPath(rgId, vaultId)}/audit`, 'GET', 'Failed to load audit log');
