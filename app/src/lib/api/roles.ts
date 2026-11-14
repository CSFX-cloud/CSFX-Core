import { api } from './http';

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

export const listRoles = () =>
    api<Role[]>('GET', '/roles');

export const listPermissions = () =>
    api<Permission[]>('GET', '/permissions');

export const createRole = (input: RoleInput) =>
    api<Role>('POST', '/roles', input);

export const updateRole = (id: string, input: RoleInput) =>
    api<Role>('PUT', `/roles/${id}`, input);

export const deleteRole = (id: string) =>
    api<void>('DELETE', `/roles/${id}`);
