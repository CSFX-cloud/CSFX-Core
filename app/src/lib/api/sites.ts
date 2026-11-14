import { api } from './http';

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
    webhook_path?: string;
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
    const root = `/resource-groups/${rgId}/sites`;
    return siteId ? `${root}/${siteId}` : root;
}

export const listSites = (rgId: string) => api<SiteSummary[]>('GET', sitesPath(rgId));

export const detectSite = (rgId: string, input: SourceInput) =>
    api<{ detection: Detection | null }>('POST', `${sitesPath(rgId)}/detect`, input);

export const createSite = (rgId: string, input: CreateSiteInput) =>
    api<Site>('POST', sitesPath(rgId), input);

export const getSite = (rgId: string, siteId: string) => api<SiteDetail>('GET', sitesPath(rgId, siteId));

export const updateSite = (rgId: string, siteId: string, input: UpdateSiteInput) =>
    api<Site>('PUT', sitesPath(rgId, siteId), input);

export const deleteSite = (rgId: string, siteId: string) => api<void>('DELETE', sitesPath(rgId, siteId));

export const redeploySite = (rgId: string, siteId: string, branch?: string) =>
    api<DeploymentSummary>('POST', `${sitesPath(rgId, siteId)}/deployments`, branch ? { branch } : {});

export const getDeployment = (rgId: string, siteId: string, deploymentId: string) =>
    api<Deployment>('GET', `${sitesPath(rgId, siteId)}/deployments/${deploymentId}`);

export const promoteDeployment = (rgId: string, siteId: string, deploymentId: string) =>
    api<void>('POST', `${sitesPath(rgId, siteId)}/deployments/${deploymentId}/promote`);

export const addSiteEnv = (rgId: string, siteId: string, input: EnvInput) =>
    api<SiteEnv>('POST', `${sitesPath(rgId, siteId)}/env`, input);

export const deleteSiteEnv = (rgId: string, siteId: string, envId: string) =>
    api<void>('DELETE', `${sitesPath(rgId, siteId)}/env/${envId}`);

export const addSiteDomain = (rgId: string, siteId: string, hostname: string) =>
    api<SiteDomain>('POST', `${sitesPath(rgId, siteId)}/domains`, { hostname });

export const deleteSiteDomain = (rgId: string, siteId: string, domainId: string) =>
    api<void>('DELETE', `${sitesPath(rgId, siteId)}/domains/${domainId}`);

export const getSiteUsage = (rgId: string, siteId: string, days: number) =>
    api<SiteUsage>('GET', `${sitesPath(rgId, siteId)}/usage?days=${days}`);
