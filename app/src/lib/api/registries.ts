import { api } from './http';

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
    const root = `/resource-groups/${rgId}/registries`;
    return registryId ? `${root}/${registryId}` : root;
}

export const listRegistries = (rgId: string) =>
    api<Registry[]>('GET', registriesPath(rgId));

export const createRegistry = (rgId: string, name: string) =>
    api<Registry>('POST', registriesPath(rgId), { name });

export const getRegistry = (rgId: string, registryId: string) =>
    api<RegistryDetail>('GET', registriesPath(rgId, registryId));

export const deleteRegistry = (rgId: string, registryId: string) =>
    api<void>('DELETE', registriesPath(rgId, registryId));

export const listImages = (rgId: string, registryId: string) =>
    api<{ repositories: Repository[] }>('GET', `${registriesPath(rgId, registryId)}/images`);

export const deleteImage = (rgId: string, registryId: string, repository: string, tag: string) =>
    api<void>('DELETE', `${registriesPath(rgId, registryId)}/images?repository=${encodeURIComponent(repository)}&tag=${encodeURIComponent(tag)}`);

export const createToken = (rgId: string, registryId: string, name: string, scope: TokenScope) =>
    api<CreatedToken>('POST', `${registriesPath(rgId, registryId)}/tokens`, { name, scope });

export const deleteToken = (rgId: string, registryId: string, tokenId: string) =>
    api<void>('DELETE', `${registriesPath(rgId, registryId)}/tokens/${tokenId}`);
