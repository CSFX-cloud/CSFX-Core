const KNOWN_IMAGE_ICONS: Record<string, string> = {
    redis: 'logos:redis',
    postgres: 'logos:postgresql',
    postgresql: 'logos:postgresql',
    nginx: 'logos:nginx',
    mysql: 'logos:mysql',
    mariadb: 'logos:mariadb',
    mongo: 'logos:mongodb',
    node: 'logos:nodejs-icon',
    mongodb: 'logos:mongodb',
    rabbitmq: 'logos:rabbitmq-icon',
    memcached: 'logos:memcached',
    traefik: 'logos:traefikproxy-icon',
    caddy: 'logos:caddy-icon',
    elasticsearch: 'logos:elasticsearch',
    grafana: 'logos:grafana',
    prometheus: 'logos:prometheus',
};

const DOCKER_FALLBACK_ICON = 'logos:docker-icon';

export function resolveImageIcon(image: string): string {
    const withoutTag = image.split('@')[0].split(':')[0];
    const segments = withoutTag.split('/');
    const bareName = segments[segments.length - 1].toLowerCase();

    return KNOWN_IMAGE_ICONS[bareName] ?? DOCKER_FALLBACK_ICON;
}

const VM_ICONS: Array<[string, string]> = [
    ['ubuntu', 'logos:ubuntu'],
    ['windows', 'logos:microsoft-windows-icon'],
    ['debian', 'logos:debian'],
    ['fedora', 'logos:fedora'],
    ['alpine', 'logos:alpinelinux-icon'],
];

const VM_FALLBACK_ICON = 'mdi:monitor';

export function resolveWorkloadIcon(image: string, runtimeClass: string): string {
    if (runtimeClass !== 'vm') return resolveImageIcon(image);
    const lowered = image.toLowerCase();
    return VM_ICONS.find(([needle]) => lowered.includes(needle))?.[1] ?? VM_FALLBACK_ICON;
}

export function resolveNodeOsIcon(osType: string): string {
    return osType.toLowerCase().includes('nix') ? 'logos:nixos' : 'logos:linux-tux';
}
