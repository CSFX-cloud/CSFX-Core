import { authedFetch } from './http';

const API_BASE = '/api';

export interface OrganizationUser {
    id: string;
    username: string;
    email: string | null;
    role_id: string;
    role_name: string;
    force_password_change: boolean;
    two_factor_enabled: boolean;
    gravatar_email: string | null;
    auth_source: string;
    joined_at: string;
}

export interface OrganizationRole {
    id: string;
    name: string;
    description: string | null;
    is_system_role: boolean;
}

function authHeaders(token: string): HeadersInit {
    return { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' };
}

export async function listUsers(token: string): Promise<OrganizationUser[]> {
    const res = await authedFetch(`${API_BASE}/organization/users`, { headers: authHeaders(token) });
    if (!res.ok) throw new Error(`Failed to list users: ${res.status}`);
    return res.json();
}

export async function listOrganizationRoles(token: string): Promise<OrganizationRole[]> {
    const res = await authedFetch(`${API_BASE}/organization/roles`, { headers: authHeaders(token) });
    if (!res.ok) throw new Error(`Failed to list roles: ${res.status}`);
    return res.json();
}

export async function updateUserRole(token: string, userId: string, roleId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/organization/users/${userId}/role`, {
        method: 'PUT',
        headers: authHeaders(token),
        body: JSON.stringify({ role_id: roleId }),
    });
    if (!res.ok) throw new Error(`Failed to update role: ${res.status}`);
}

export async function deleteUser(token: string, userId: string): Promise<void> {
    const res = await authedFetch(`${API_BASE}/organization/users/${userId}`, {
        method: 'DELETE',
        headers: authHeaders(token),
    });
    if (!res.ok) throw new Error(`Failed to delete user: ${res.status}`);
}
