import { authedFetch } from './http';

const API_BASE = '/api';

export interface Permission {
    id: string;
    name: string;
    resource: string;
    action: string;
    description: string | null;
}

export interface Role {
    id: string;
    name: string;
    description: string | null;
    is_system_role: boolean;
    permission_ids: string[];
}

export interface RoleInput {
    name: string;
    description: string | null;
    permission_ids: string[];
}

async function failure(res: Response, fallback: string): Promise<Error> {
    const body = await res.json().catch(() => ({}));
    return new Error(body.error ?? `${fallback}: ${res.status}`);
}

function authHeaders(token: string): HeadersInit {
    return { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' };
}

export async function listRoles(token: string): Promise<Role[]> {
    const res = await authedFetch(`${API_BASE}/roles`, { headers: authHeaders(token) });
    if (!res.ok) throw await failure(res, 'Failed to list roles');
    return res.json();
}

export async function listPermissions(token: string): Promise<Permission[]> {
    const res = await authedFetch(`${API_BASE}/permissions`, { headers: authHeaders(token) });
    if (!res.ok) throw await failure(res, 'Failed to list permissions');
    return res.json();
}

export async function createRole(token: string, input: RoleInput): Promise<Role> {
    const res = await authedFetch(`${API_BASE}/roles`, {
        method: 'POST',
        headers: authHeaders(token),
        body: JSON.stringify(input),
    });
    if (!res.ok) throw await failure(res, 'Failed to create role');
    return res.json();
}

export async function updateRole(token: string, id: string, input: RoleInput): Promise<Role> {
    const res = await authedFetch(`${API_BASE}/roles/${id}`, {
        method: 'PUT',
        headers: authHeaders(token),
        body: JSON.stringify(input),
    });
    if (!res.ok) throw await failure(res, 'Failed to update role');
    return res.json();
}

export async function deleteRole(token: string, id: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/roles/${id}`, {
        method: 'DELETE',
        headers: authHeaders(token),
    });
    if (!res.ok) throw await failure(res, 'Failed to delete role');
}
