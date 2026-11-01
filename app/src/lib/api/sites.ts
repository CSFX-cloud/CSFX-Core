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
}

export interface Deployment extends Omit<DeploymentSummary, 'current'> {
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
    kind: 'production' | 'branch' | 'auto';
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

export interface SourceInput {
    repo_url: string;
    branch?: string;
    root_dir?: string;
    vault_id?: string;
    git_token_secret?: string;
}

export interface CreateSiteInput extends SourceInput {
    name: string;
    gateway_id: string;
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
