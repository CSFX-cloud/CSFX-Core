import { error } from '@sveltejs/kit';
import type { Handle } from '@sveltejs/kit/hooks';
import { API_GATEWAY_URL } from '$app/env/private';
import type { UserProfile } from '$lib/auth/api';

const SESSION_COOKIE = 'auth_token';

async function validateSession(token: string): Promise<UserProfile | null> {
    const response = await fetch(`${API_GATEWAY_URL}/api/validate-session`, {
        headers: { cookie: `${SESSION_COOKIE}=${token}` },
    });
    if (response.status === 401) return null;
    if (!response.ok) error(502, 'Session validation failed');
    return response.json();
}

function isAssetRequest(pathname: string): boolean {
    return pathname.startsWith('/_app/') || /\.[a-z0-9]+$/i.test(pathname);
}

export const handle: Handle = async ({ event, resolve }) => {
    const token = event.cookies.get(SESSION_COOKIE);
    event.locals.user = token && !isAssetRequest(event.url.pathname) ? await validateSession(token) : null;
    return resolve(event);
};
