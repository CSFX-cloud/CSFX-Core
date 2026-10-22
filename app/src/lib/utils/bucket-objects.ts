export interface PrefixCrumb {
    name: string;
    prefix: string;
}

export function folderName(prefix: string): string {
    const parts = prefix.slice(0, -1).split("/");
    return parts[parts.length - 1];
}

export function objectName(key: string): string {
    const parts = key.split("/");
    return parts[parts.length - 1];
}

export function prefixCrumbs(prefix: string): PrefixCrumb[] {
    if (!prefix) return [];
    let accumulated = "";
    return prefix
        .slice(0, -1)
        .split("/")
        .map((segment) => {
            accumulated += `${segment}/`;
            return { name: segment, prefix: accumulated };
        });
}
