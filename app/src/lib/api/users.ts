import { api, ApiError } from './http';

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

export const listUsers = () =>
    api<OrganizationUser[]>('GET', '/organization/users');

export const listOrganizationRoles = () =>
    api<OrganizationRole[]>('GET', '/organization/roles');

export interface CreateUserInput {
    username: string;
    email: string | null;
    password: string;
    role_id: string;
    force_password_change: boolean;
}

export async function createUser(input: CreateUserInput): Promise<OrganizationUser> {
    try {
        return await api<OrganizationUser>('POST', '/organization/users', input);
    } catch (e) {
        if (e instanceof ApiError && e.status === 409) throw new Error('Username or email already in use');
        throw e;
    }
}

export const updateUserRole = (userId: string, roleId: string) =>
    api<void>('PUT', `/organization/users/${userId}/role`, { role_id: roleId });

export const deleteUser = (userId: string) =>
    api<void>('DELETE', `/organization/users/${userId}`);
