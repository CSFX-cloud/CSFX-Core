export function scopeMatches(scope: string, name: string): boolean {
    return scope.endsWith("*") ? name.startsWith(scope.slice(0, -1)) : scope === name;
}
