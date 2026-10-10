import { authedFetch } from './http';

const API_BASE = '/api';

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

async function failure(res: Response, fallback: string): Promise<Error> {
    const body = await res.json().catch(() => ({}));
    return new Error(body.error ?? `${fallback}: ${res.status}`);
}

function authHeaders(token: string): HeadersInit {
    return { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' };
}

export async function listIdentityProviders(token: string): Promise<IdentityProvider[]> {
    const res = await authedFetch(`${API_BASE}/identity-providers`, { headers: authHeaders(token) });
    if (!res.ok) throw await failure(res, 'Failed to list identity providers');
    return res.json();
}

export async function createIdentityProvider(
    token: string,
    slug: string,
    input: IdentityProviderInput,
): Promise<IdentityProvider> {
    const res = await authedFetch(`${API_BASE}/identity-providers`, {
        method: 'POST',
        headers: authHeaders(token),
        body: JSON.stringify({ slug, ...input }),
    });
    if (!res.ok) throw await failure(res, 'Failed to create identity provider');
    return res.json();
}

export async function updateIdentityProvider(
    token: string,
    id: string,
    input: IdentityProviderInput,
): Promise<IdentityProvider> {
    const res = await authedFetch(`${API_BASE}/identity-providers/${id}`, {
        method: 'PUT',
        headers: authHeaders(token),
        body: JSON.stringify(input),
    });
    if (!res.ok) throw await failure(res, 'Failed to update identity provider');
    return res.json();
}

export async function deleteIdentityProvider(token: string, id: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/identity-providers/${id}`, {
        method: 'DELETE',
        headers: authHeaders(token),
    });
    if (!res.ok) throw await failure(res, 'Failed to delete identity provider');
}

export async function testIdentityProvider(token: string, id: string): Promise<DiscoveryResult> {
    const res = await authedFetch(`${API_BASE}/identity-providers/${id}/test`, {
        method: 'POST',
        headers: authHeaders(token),
    });
    if (!res.ok) throw await failure(res, 'Discovery failed');
    return res.json();
}

export async function listGroupMappings(token: string, id: string): Promise<GroupMapping[]> {
    const res = await authedFetch(`${API_BASE}/identity-providers/${id}/group-mappings`, {
        headers: authHeaders(token),
    });
    if (!res.ok) throw await failure(res, 'Failed to list group mappings');
    return res.json();
}

export async function replaceGroupMappings(
    token: string,
    id: string,
    mappings: GroupMapping[],
): Promise<void> {
    const res = await authedFetch(`${API_BASE}/identity-providers/${id}/group-mappings`, {
        method: 'PUT',
        headers: authHeaders(token),
        body: JSON.stringify(mappings),
    });
    if (!res.ok) throw await failure(res, 'Failed to save group mappings');
}
