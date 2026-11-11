import type { AuthResponse, UserProfile } from './api';

interface AuthState {
    token: string | null;
    user: UserProfile | null;
}

function createAuthStore() {
    const TOKEN_KEY = 'csfx_token';

    let state = $state<AuthState>({
        token: null,
        user: null,
    });

    function init() {
        if (typeof localStorage === 'undefined') return;
        const stored = localStorage.getItem(TOKEN_KEY);
        if (stored) state.token = stored;
    }

    function setUser(profile: UserProfile) {
        state.user = profile;
    }

    function setSession(response: AuthResponse, profile?: UserProfile) {
        state.token = response.token;
        if (profile) {
            state.user = profile;
        } else {
            state.user = {
                id: response.user_id,
                username: response.username,
                email: null,
                gravatar_email: null,
                two_factor_enabled: response.two_factor_enabled,
                force_password_change: response.force_password_change,
            };
        }
        localStorage.setItem(TOKEN_KEY, response.token);
    }

    function setToken(token: string) {
        state.token = token;
        localStorage.setItem(TOKEN_KEY, token);
    }

    function clearSession() {
        state.token = null;
        state.user = null;
        localStorage.removeItem(TOKEN_KEY);
    }

    return {
        get token() { return state.token; },
        get user() { return state.user; },
        init,
        setSession,
        setUser,
        setToken,
        clearSession,
    };
}

export const auth = createAuthStore();
