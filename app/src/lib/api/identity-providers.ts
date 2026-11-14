import { api } from './http';

export interface IdentityProvider {
    id: string;
    slug: string;
    display_name: string;
    issuer_url: string;
    client_id: string;
    scopes: string;
    username_claim: string;
    email_claim: string;
    groups_claim: string;
    default_role_id: string | null;
    auto_provision: boolean;
    enabled: boolean;
    redirect_uri: string;
    user_count: number;
}

export interface IdentityProviderInput {
    display_name: string;
    issuer_url: string;
    client_id: string;
    client_secret?: string;
    scopes: string;
    username_claim: string;
    email_claim: string;
    groups_claim: string;
    default_role_id: string | null;
    auto_provision: boolean;
    enabled: boolean;
}

export interface GroupMapping {
    external_group: string;
    role_id: string;
    priority: number;
}

export interface DiscoveryResult {
    issuer: string;
    authorization_endpoint: string;
    token_endpoint: string;
    jwks_uri: string;
}

export const listIdentityProviders = () =>
    api<IdentityProvider[]>('GET', '/identity-providers');

export const createIdentityProvider = (slug: string, input: IdentityProviderInput) =>
    api<IdentityProvider>('POST', '/identity-providers', { slug, ...input });

export const updateIdentityProvider = (id: string, input: IdentityProviderInput) =>
    api<IdentityProvider>('PUT', `/identity-providers/${id}`, input);

export const deleteIdentityProvider = (id: string, deleteUsers: boolean) =>
    api<void>('DELETE', `/identity-providers/${id}?delete_users=${deleteUsers}`);

export const testIdentityProvider = (id: string) =>
    api<DiscoveryResult>('POST', `/identity-providers/${id}/test`);

export const listGroupMappings = (id: string) =>
    api<GroupMapping[]>('GET', `/identity-providers/${id}/group-mappings`);

export const replaceGroupMappings = (id: string, mappings: GroupMapping[]) =>
    api<void>('PUT', `/identity-providers/${id}/group-mappings`, mappings);
