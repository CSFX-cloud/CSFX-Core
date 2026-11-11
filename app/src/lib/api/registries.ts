import { authedFetch } from './http';

const API_BASE = '/api';

export type TokenScope = 'pull' | 'push';

export interface Registry {
    id: string;
    resource_group_id: string;
    name: string;
    prefix: string;
    created_at: string;
}

export interface RegistryToken {
    id: string;
    name: string;
    username: string;
    scope: TokenScope;
    created_at: string;
    last_used_at: string | null;
}

export interface RegistryDetail extends Registry {
    tokens: RegistryToken[];
}

export interface CreatedToken extends RegistryToken {
    secret: string;
}

export interface ImageTag {
    tag: string;
    digest: string | null;
    size_bytes: number | null;
}

export interface Repository {
    repository: string;
    tags: ImageTag[];
}

function registriesPath(rgId: string, registryId?: string): string {
    const root = `${API_BASE}/resource-groups/${rgId}/registries`;
    return registryId ? `${root}/${registryId}` : root;
}

async function send<T>(token: string, url: string, method: string, fallback: string, body?: unknown): Promise<T> {
    const res = await authedFetch(url, {
        method,
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) {
        const payload = await res.json().catch(() => ({}));
        throw new Error(payload.error ?? `${fallback}: ${res.status}`);
    }
    return res.status === 204 ? (undefined as T) : res.json();
}

export const listRegistries = (token: string, rgId: string) =>
    send<Registry[]>(token, registriesPath(rgId), 'GET', 'Failed to list registries');

export const createRegistry = (token: string, rgId: string, name: string) =>
    send<Registry>(token, registriesPath(rgId), 'POST', 'Failed to create registry', { name });

export const getRegistry = (token: string, rgId: string, registryId: string) =>
    send<RegistryDetail>(token, registriesPath(rgId, registryId), 'GET', 'Failed to load registry');

export const deleteRegistry = (token: string, rgId: string, registryId: string) =>
    send<void>(token, registriesPath(rgId, registryId), 'DELETE', 'Failed to delete registry');

export const listImages = (token: string, rgId: string, registryId: string) =>
    send<{ repositories: Repository[] }>(token, `${registriesPath(rgId, registryId)}/images`, 'GET', 'Failed to list images');

export const deleteImage = (token: string, rgId: string, registryId: string, repository: string, tag: string) =>
    send<void>(
        token,
        `${registriesPath(rgId, registryId)}/images?repository=${encodeURIComponent(repository)}&tag=${encodeURIComponent(tag)}`,
        'DELETE',
        'Failed to delete image',
    );

export const createToken = (token: string, rgId: string, registryId: string, name: string, scope: TokenScope) =>
    send<CreatedToken>(token, `${registriesPath(rgId, registryId)}/tokens`, 'POST', 'Failed to create access token', { name, scope });

export const deleteToken = (token: string, rgId: string, registryId: string, tokenId: string) =>
    send<void>(token, `${registriesPath(rgId, registryId)}/tokens/${tokenId}`, 'DELETE', 'Failed to revoke access token');
