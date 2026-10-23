import { browser } from "$app/environment";

export type ThemePreference = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

export const THEME_STORAGE_KEY = "csfx-theme";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function isPreference(value: string | null): value is ThemePreference {
    return value === "system" || value === "light" || value === "dark";
}

function readPreference(): ThemePreference {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isPreference(stored) ? stored : "dark";
}

function resolve(preference: ThemePreference): ResolvedTheme {
    if (preference !== "system") return preference;
    return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

function createTheme() {
    let preference = $state<ThemePreference>(browser ? readPreference() : "dark");
    let resolved = $state<ResolvedTheme>(browser ? resolve(preference) : "dark");

    function apply() {
        resolved = resolve(preference);
        const root = document.documentElement;
        root.dataset.theme = resolved;
        root.classList.toggle("dark", resolved === "dark");
    }

    function set(next: ThemePreference) {
        preference = next;
        localStorage.setItem(THEME_STORAGE_KEY, next);
        apply();
    }

    function init() {
        apply();
        const query = window.matchMedia(DARK_QUERY);
        query.addEventListener("change", apply);
        return () => query.removeEventListener("change", apply);
    }

    return {
        get preference() {
            return preference;
        },
        get resolved() {
            return resolved;
        },
        set,
        init,
    };
}

export const theme = createTheme();
