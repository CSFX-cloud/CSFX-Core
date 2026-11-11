import { authedFetch } from './http';

const API_BASE = '/api';

export type SiteMode = 'static' | 'container';
export type DeploymentStatus = 'queued' | 'building' | 'ready' | 'failed' | 'rolled_back';
export type EnvScope = 'all' | 'production' | 'preview';

export interface Site {
    id: string;
    resource_group_id: string;
    name: string;
    mode: SiteMode;
    repo_url: string;
    production_branch: string;
    root_dir: string;
    framework: string | null;
    install_command: string | null;
    build_command: string | null;
    output_dir: string | null;
    start_command: string | null;
    container_port: number | null;
    vault_id: string | null;
    gateway_id: string | null;
    git_token_secret: string | null;
    preview_enabled: boolean;
    current_deployment_id: string | null;
    created_at: string;
}

export interface DeploymentSummary {
    id: string;
    commit_sha: string;
    commit_message: string;
    branch: string;
    pull_request: number | null;
    environment: 'production' | 'preview';
    status: DeploymentStatus;
    duration_ms: number | null;
    created_at: string;
    finished_at: string | null;
    current: boolean;
    expired: boolean;
}

export interface Deployment extends Omit<DeploymentSummary, 'current' | 'expired'> {
    site_id: string;
    build_log: string;
    image_ref: string | null;
}

export interface SiteSummary extends Site {
    domains: string[];
    current: DeploymentSummary | null;
    latest: DeploymentSummary | null;
}

export interface SiteDomain {
    id: string;
    hostname: string;
    kind: 'production' | 'branch' | 'auto' | 'internal';
    branch: string | null;
}

export interface SiteEnv {
    id: string;
    name: string;
    value: string | null;
    vault_secret: string | null;
    scope: EnvScope;
}

export interface SiteDetail extends Site {
    webhook_path: string;
    domains: SiteDomain[];
    env: SiteEnv[];
    deployments: DeploymentSummary[];
    internal_port: number | null;
}

export interface Detection {
    framework: string | null;
    mode: SiteMode;
    install_command: string | null;
    build_command: string | null;
    output_dir: string | null;
    start_command: string | null;
    port: number | null;
}

export interface FrameworkPreset extends Detection {
    framework: string;
    label: string;
}

export const FRAMEWORK_PRESETS: FrameworkPreset[] = [
    { framework: 'sveltekit', label: 'SvelteKit (Node adapter)', mode: 'container', install_command: 'npm ci', build_command: 'npm run build', output_dir: null, start_command: 'node build', port: 3000 },
    { framework: 'sveltekit-static', label: 'SvelteKit (static adapter)', mode: 'static', install_command: 'npm ci', build_command: 'npm run build', output_dir: 'build', start_command: null, port: null },
    { framework: 'next', label: 'Next.js', mode: 'container', install_command: 'npm ci', build_command: 'npm run build', output_dir: null, start_command: 'npm run start', port: 3000 },
    { framework: 'nuxt', label: 'Nuxt', mode: 'container', install_command: 'npm ci', build_command: 'npm run build', output_dir: null, start_command: 'node .output/server/index.mjs', port: 3000 },
    { framework: 'astro', label: 'Astro (static)', mode: 'static', install_command: 'npm ci', build_command: 'npm run build', output_dir: 'dist', start_command: null, port: null },
    { framework: 'astro-server', label: 'Astro (Node adapter)', mode: 'container', install_command: 'npm ci', build_command: 'npm run build', output_dir: null, start_command: 'node ./dist/server/entry.mjs', port: 4321 },
    { framework: 'vite', label: 'Vite', mode: 'static', install_command: 'npm ci', build_command: 'npm run build', output_dir: 'dist', start_command: null, port: null },
    { framework: 'create-react-app', label: 'Create React App', mode: 'static', install_command: 'npm ci', build_command: 'npm run build', output_dir: 'build', start_command: null, port: null },
    { framework: 'docusaurus', label: 'Docusaurus', mode: 'static', install_command: 'npm ci', build_command: 'npm run build', output_dir: 'build', start_command: null, port: null },
    { framework: 'hugo', label: 'Hugo', mode: 'static', install_command: null, build_command: 'hugo --minify', output_dir: 'public', start_command: null, port: null },
    { framework: 'static', label: 'Plain HTML', mode: 'static', install_command: null, build_command: null, output_dir: '.', start_command: null, port: null },
    { framework: 'dockerfile', label: 'Dockerfile', mode: 'container', install_command: null, build_command: null, output_dir: null, start_command: null, port: null },
];

export type PackageManager = 'npm' | 'pnpm' | 'yarn' | 'bun';

export const PACKAGE_MANAGERS: PackageManager[] = ['npm', 'pnpm', 'yarn', 'bun'];

const MANAGER_COMMANDS: Record<PackageManager, { install: string; run: string }> = {
    npm: { install: 'npm ci', run: 'npm run' },
    pnpm: { install: 'pnpm install --frozen-lockfile', run: 'pnpm run' },
    yarn: { install: 'yarn install --frozen-lockfile', run: 'yarn' },
    bun: { install: 'bun install --frozen-lockfile', run: 'bun run' },
};

export function detectPackageManager(installCommand: string | null): PackageManager {
    return PACKAGE_MANAGERS.find((manager) => installCommand?.startsWith(manager)) ?? 'npm';
}

function retarget(command: string | null, manager: PackageManager): string | null {
    if (!command) return command;
    const target = MANAGER_COMMANDS[manager];
    if (command.startsWith('npm ci')) return target.install;
    return command.startsWith('npm run ') ? `${target.run} ${command.slice('npm run '.length)}` : command;
}

export function withPackageManager(preset: Detection, manager: PackageManager): Detection {
    return {
        ...preset,
        install_command: retarget(preset.install_command, manager),
        build_command: retarget(preset.build_command, manager),
        start_command: retarget(preset.start_command, manager),
    };
}

export interface SourceInput {
    repo_url: string;
    branch?: string;
    root_dir?: string;
    vault_id?: string;
    git_token_secret?: string;
}

export interface CreateSiteInput extends SourceInput {
    name: string;
    gateway_id?: string;
    preview_enabled?: boolean;
    mode?: SiteMode;
    framework?: string;
    install_command?: string;
    build_command?: string;
    output_dir?: string;
    start_command?: string;
    container_port?: number;
}

export type UpdateSiteInput = Partial<
    Pick<
        Site,
        'production_branch' | 'root_dir' | 'framework' | 'install_command' | 'build_command' | 'output_dir' | 'start_command' | 'container_port' | 'vault_id' | 'git_token_secret' | 'gateway_id' | 'preview_enabled'
    >
>;

export interface DailyUsage {
    date: string;
    requests: number;
    bytes_out: number;
}

export interface SiteUsage {
    days: number;
    requests: number;
    bytes_out: number;
    build_minutes: number;
    series: DailyUsage[];
}

export interface EnvInput {
    name: string;
    value?: string;
    vault_secret?: string;
    scope: EnvScope;
}

function sitesPath(rgId: string, siteId?: string): string {
    const root = `${API_BASE}/resource-groups/${rgId}/sites`;
    return siteId ? `${root}/${siteId}` : root;
}

async function send<T>(token: string, url: string, method: string, fallback: string, body?: unknown): Promise<T> {
    const res = await authedFetch(url, {
        method,
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) {
        const payload = await res.json().catch(() => ({}));
        throw new Error(payload.error ?? `${fallback}: ${res.status}`);
    }
    return res.status === 204 ? (undefined as T) : res.json();
}

export const listSites = (token: string, rgId: string) => send<SiteSummary[]>(token, sitesPath(rgId), 'GET', 'Failed to list web apps');

export const detectSite = (token: string, rgId: string, input: SourceInput) =>
    send<{ detection: Detection | null }>(token, `${sitesPath(rgId)}/detect`, 'POST', 'Failed to inspect the repository', input);

export const createSite = (token: string, rgId: string, input: CreateSiteInput) =>
    send<Site>(token, sitesPath(rgId), 'POST', 'Failed to create web app', input);

export const getSite = (token: string, rgId: string, siteId: string) => send<SiteDetail>(token, sitesPath(rgId, siteId), 'GET', 'Failed to load web app');

export const updateSite = (token: string, rgId: string, siteId: string, input: UpdateSiteInput) =>
    send<Site>(token, sitesPath(rgId, siteId), 'PUT', 'Failed to update web app', input);

export const deleteSite = (token: string, rgId: string, siteId: string) => send<void>(token, sitesPath(rgId, siteId), 'DELETE', 'Failed to delete web app');

export const redeploySite = (token: string, rgId: string, siteId: string, branch?: string) =>
    send<DeploymentSummary>(token, `${sitesPath(rgId, siteId)}/deployments`, 'POST', 'Failed to start deployment', branch ? { branch } : {});

export const getDeployment = (token: string, rgId: string, siteId: string, deploymentId: string) =>
    send<Deployment>(token, `${sitesPath(rgId, siteId)}/deployments/${deploymentId}`, 'GET', 'Failed to load deployment');

export const promoteDeployment = (token: string, rgId: string, siteId: string, deploymentId: string) =>
    send<void>(token, `${sitesPath(rgId, siteId)}/deployments/${deploymentId}/promote`, 'POST', 'Failed to promote deployment');

export const addSiteEnv = (token: string, rgId: string, siteId: string, input: EnvInput) =>
    send<SiteEnv>(token, `${sitesPath(rgId, siteId)}/env`, 'POST', 'Failed to add variable', input);

export const deleteSiteEnv = (token: string, rgId: string, siteId: string, envId: string) =>
    send<void>(token, `${sitesPath(rgId, siteId)}/env/${envId}`, 'DELETE', 'Failed to remove variable');

export const addSiteDomain = (token: string, rgId: string, siteId: string, hostname: string) =>
    send<SiteDomain>(token, `${sitesPath(rgId, siteId)}/domains`, 'POST', 'Failed to add domain', { hostname });

export const deleteSiteDomain = (token: string, rgId: string, siteId: string, domainId: string) =>
    send<void>(token, `${sitesPath(rgId, siteId)}/domains/${domainId}`, 'DELETE', 'Failed to remove domain');

export const getSiteUsage = (token: string, rgId: string, siteId: string, days: number) =>
    send<SiteUsage>(token, `${sitesPath(rgId, siteId)}/usage?days=${days}`, 'GET', 'Failed to load usage');
