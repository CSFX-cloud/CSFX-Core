import { goto } from '$app/navigation';
import { auth } from '$lib/auth/store.svelte';

const API_BASE = '/api';

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

export class ApiError extends Error {
    constructor(
        readonly status: number,
        message: string,
    ) {
        super(message);
    }
}

async function errorMessage(res: Response): Promise<string> {
    const payload = await res.json().catch(() => ({}));
    return typeof payload.error === 'string' ? payload.error : `Request failed: ${res.status}`;
}

function requestInit(method: HttpMethod, token: string | null, body: unknown, signal?: AbortSignal): RequestInit {
    const headers: Record<string, string> = {};
    if (token) headers.Authorization = `Bearer ${token}`;
    if (body instanceof Blob) {
        headers['Content-Type'] = body.type;
        return { method, headers, body, signal };
    }
    if (body === undefined) return { method, headers, signal };
    headers['Content-Type'] = 'application/json';
    return { method, headers, body: JSON.stringify(body), signal };
}

export async function request(method: HttpMethod, path: string, body?: unknown, signal?: AbortSignal): Promise<Response> {
    const token = auth.token;
    const res = await fetch(`${API_BASE}${path}`, requestInit(method, token, body, signal));
    if (res.status === 401 && token) {
        auth.clearSession();
        goto('/login');
    }
    if (!res.ok) throw new ApiError(res.status, await errorMessage(res));
    return res;
}

export async function api<T>(method: HttpMethod, path: string, body?: unknown): Promise<T> {
    const res = await request(method, path, body);
    const text = await res.text();
    return (text ? JSON.parse(text) : undefined) as T;
}

export async function openSocketUrl(ticketPath: string, streamPath: string): Promise<string> {
    const { ticket } = await api<{ ticket: string }>('POST', ticketPath);
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${API_BASE}${streamPath}?ticket=${encodeURIComponent(ticket)}`;
}
