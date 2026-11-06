export interface ComposePreviewService {
    serviceName: string;
    image: string | null;
    ports: string[];
}

export interface ComposePreviewResult {
    services: ComposePreviewService[];
    volumes: string[];
    error: string | null;
}

function stripInlineComment(line: string): string {
    const hashIndex = line.indexOf('#');
    return hashIndex === -1 ? line : line.slice(0, hashIndex);
}

function indentOf(line: string): number {
    return line.length - line.trimStart().length;
}

function parseTopLevelBlockKeys(lines: string[], blockName: string): string[] {
    const blockLineIndex = lines.findIndex((line) => line.trimEnd() === `${blockName}:`);
    if (blockLineIndex === -1) return [];

    const keys: string[] = [];
    for (let i = blockLineIndex + 1; i < lines.length; i++) {
        const line = lines[i];
        if (!line.trim()) continue;
        const indent = indentOf(line);
        if (indent === 0) break;
        if (indent === 2 && line.trim().endsWith(':')) {
            keys.push(line.trim().slice(0, -1));
        }
    }
    return keys;
}

export function parseComposePreview(yaml: string): ComposePreviewResult {
    const lines = yaml.split('\n').map(stripInlineComment);
    const volumes = parseTopLevelBlockKeys(lines, 'volumes');
    const servicesLineIndex = lines.findIndex((line) => line.trim() === 'services:');
    if (servicesLineIndex === -1) {
        return { services: [], volumes, error: null };
    }

    const services: ComposePreviewService[] = [];
    let current: ComposePreviewService | null = null;
    let inPorts = false;

    for (let i = servicesLineIndex + 1; i < lines.length; i++) {
        const line = lines[i];
        if (!line.trim()) continue;
        const indent = indentOf(line);
        const trimmed = line.trim();

        if (indent === 0) break;

        if (indent === 2 && trimmed.endsWith(':')) {
            current = { serviceName: trimmed.slice(0, -1), image: null, ports: [] };
            services.push(current);
            inPorts = false;
            continue;
        }
        if (indent <= 0 || !current) continue;

        if (trimmed.startsWith('image:')) {
            current.image = trimmed.slice('image:'.length).trim().replace(/^["']|["']$/g, '');
            inPorts = false;
        } else if (trimmed === 'ports:') {
            inPorts = true;
        } else if (inPorts && trimmed.startsWith('-')) {
            current.ports.push(trimmed.slice(1).trim().replace(/^["']|["']$/g, ''));
        } else if (trimmed.endsWith(':')) {
            inPorts = false;
        }
    }

    return { services, volumes, error: null };
}
