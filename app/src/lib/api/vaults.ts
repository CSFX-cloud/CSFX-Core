import { api } from './http';

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
    const root = `/resource-groups/${rgId}/vaults`;
    return vaultId ? `${root}/${vaultId}` : root;
}

export const listVaults = (rgId: string) =>
    api<Vault[]>('GET', vaultsPath(rgId));

export const createVault = (rgId: string, name: string) =>
    api<Vault>('POST', vaultsPath(rgId), { name });

export const getVault = (rgId: string, vaultId: string) =>
    api<VaultOverview>('GET', vaultsPath(rgId, vaultId));

export const deleteVault = (rgId: string, vaultId: string) =>
    api<void>('DELETE', vaultsPath(rgId, vaultId));

export const setVaultSealed = (rgId: string, vaultId: string, sealed: boolean) =>
    api<Vault>('POST', `${vaultsPath(rgId, vaultId)}/${sealed ? 'seal' : 'unseal'}`);

export const listSecrets = (rgId: string, vaultId: string) =>
    api<SecretSummary[]>('GET', `${vaultsPath(rgId, vaultId)}/secrets`);

export const createSecret = (rgId: string, vaultId: string, input: SecretInput) =>
    api<{ name: string; version: number }>('POST', `${vaultsPath(rgId, vaultId)}/secrets`, input);

export const updateSecret = (rgId: string, vaultId: string, name: string, input: SecretInput) =>
    api<{ name: string; version: number }>('PUT', `${vaultsPath(rgId, vaultId)}/secrets/${name}`, input);

export const deleteSecret = (rgId: string, vaultId: string, name: string) =>
    api<void>('DELETE', `${vaultsPath(rgId, vaultId)}/secrets/${name}`);

export const restoreSecret = (rgId: string, vaultId: string, name: string, version: number) =>
    api<void>('POST', `${vaultsPath(rgId, vaultId)}/secrets/${name}/restore`, { version });

export const revealSecret = (rgId: string, vaultId: string, name: string, input: RevealInput) =>
    api<RevealedSecret>('POST', `${vaultsPath(rgId, vaultId)}/secrets/${name}/reveal`, input);

export const listGrants = (rgId: string, vaultId: string) =>
    api<VaultGrant[]>('GET', `${vaultsPath(rgId, vaultId)}/grants`);

export const createGrant = (rgId: string, vaultId: string, input: GrantInput) =>
    api<VaultGrant>('POST', `${vaultsPath(rgId, vaultId)}/grants`, input);

export const deleteGrant = (rgId: string, vaultId: string, grantId: string) =>
    api<void>('DELETE', `${vaultsPath(rgId, vaultId)}/grants/${grantId}`);

export const listVaultAudit = (rgId: string, vaultId: string) =>
    api<VaultAuditEntry[]>('GET', `${vaultsPath(rgId, vaultId)}/audit`);
