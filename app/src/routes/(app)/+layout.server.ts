import { redirect } from '@sveltejs/kit';
import { loginUrl } from '$lib/auth/next-path';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = ({ locals, url }) => {
    if (!locals.user) redirect(303, loginUrl(url.pathname + url.search));
};
