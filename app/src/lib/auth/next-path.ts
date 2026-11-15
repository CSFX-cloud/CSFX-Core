export function safeNextPath(value: string | null, fallback = '/'): string {
    if (!value || !value.startsWith('/') || value.startsWith('//') || value.includes('\\')) return fallback;
    return value;
}

export function loginUrl(path: string): string {
    return path === '/' ? '/login' : `/login?next=${encodeURIComponent(path)}`;
}
