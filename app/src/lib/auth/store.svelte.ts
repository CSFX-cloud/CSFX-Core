import { page } from '$app/state';
import type { UserProfile } from './api';

function createAuthStore() {
    let override = $state<UserProfile | null>(null);

    return {
        get user(): UserProfile | null {
            return override ?? page.data.user ?? null;
        },
        setUser(profile: UserProfile) {
            override = profile;
        },
    };
}

export const auth = createAuthStore();
