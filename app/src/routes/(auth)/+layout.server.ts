import { redirect } from '@sveltejs/kit';
import { safeNextPath } from '$lib/auth/next-path';
import type { LayoutServerLoad } from './$types';

const SESSION_OPTIONAL_PATHS = ['/pw_change', '/login/callback'];

export const load: LayoutServerLoad = ({ locals, url }) => {
    if (locals.user && !SESSION_OPTIONAL_PATHS.includes(url.pathname)) {
        redirect(303, safeNextPath(url.searchParams.get('next')));
    }
};
