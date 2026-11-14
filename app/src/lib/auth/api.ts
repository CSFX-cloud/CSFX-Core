import { api, ApiError } from '$lib/api/http';

const API_BASE = '/api';

export interface AuthResponse {
    token: string;
    user_id: string;
    username: string;
    two_factor_enabled: boolean;
    force_password_change: boolean;
}

export interface UserProfile {
    id: string;
    username: string;
    email: string | null;
    gravatar_email: string | null;
    two_factor_enabled: boolean;
    force_password_change: boolean;
}

async function fetchPublicKeyPem(): Promise<string> {
    const data = await api<{ public_key: string }>('GET', '/public-key');
    return data.public_key;
}

function pkcs1PemToSpki(pem: string): ArrayBuffer {
    const b64 = pem
        .replace('-----BEGIN RSA PUBLIC KEY-----', '')
        .replace('-----END RSA PUBLIC KEY-----', '')
        .replace(/\s/g, '');
    const pkcs1 = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

    // SPKI header for RSA with SHA-256: OID 1.2.840.113549.1.1.1
    const spkiHeader = new Uint8Array([
        0x30, 0x0d,
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01,
        0x05, 0x00,
    ]);

    // BIT STRING wrapper: tag 0x03, length = pkcs1.length + 1, leading 0x00
    const bitStringBody = new Uint8Array(pkcs1.length + 1);
    bitStringBody[0] = 0x00;
    bitStringBody.set(pkcs1, 1);

    const bitStringLen = bitStringBody.length;
    const bitStringLenBytes = encodeAsn1Length(bitStringLen);
    const bitString = new Uint8Array(1 + bitStringLenBytes.length + bitStringLen);
    bitString[0] = 0x03;
    bitString.set(bitStringLenBytes, 1);
    bitString.set(bitStringBody, 1 + bitStringLenBytes.length);

    const spkiBody = new Uint8Array(spkiHeader.length + bitString.length);
    spkiBody.set(spkiHeader);
    spkiBody.set(bitString, spkiHeader.length);

    const spkiBodyLen = encodeAsn1Length(spkiBody.length);
    const spki = new Uint8Array(1 + spkiBodyLen.length + spkiBody.length);
    spki[0] = 0x30;
    spki.set(spkiBodyLen, 1);
    spki.set(spkiBody, 1 + spkiBodyLen.length);

    return spki.buffer;
}

function encodeAsn1Length(len: number): Uint8Array {
    if (len < 0x80) return new Uint8Array([len]);
    if (len < 0x100) return new Uint8Array([0x81, len]);
    return new Uint8Array([0x82, (len >> 8) & 0xff, len & 0xff]);
}

async function importRsaKey(pem: string): Promise<CryptoKey> {
    const spki = pkcs1PemToSpki(pem);
    return crypto.subtle.importKey(
        'spki',
        spki,
        { name: 'RSA-OAEP', hash: 'SHA-256' },
        false,
        ['encrypt'],
    );
}

async function encryptPassword(password: string, publicKey: CryptoKey): Promise<string> {
    const encoded = new TextEncoder().encode(password);
    const encrypted = await crypto.subtle.encrypt({ name: 'RSA-OAEP' }, publicKey, encoded);
    const bytes = new Uint8Array(encrypted);
    let binary = '';
    for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
}

export async function encryptForServer(...values: string[]): Promise<string[]> {
    const publicKey = await importRsaKey(await fetchPublicKeyPem());
    return Promise.all(values.map((value) => encryptPassword(value, publicKey)));
}

export async function login(
    username: string,
    password: string,
    twoFactorCode?: string,
): Promise<AuthResponse> {
    const [encryptedPassword] = await encryptForServer(password);
    try {
        return await api<AuthResponse>('POST', '/login', {
            username,
            encrypted_password: encryptedPassword,
            two_factor_code: twoFactorCode ?? null,
        });
    } catch (e) {
        if (e instanceof ApiError && e.status === 403) throw new TwoFactorRequiredError(username, password);
        throw e;
    }
}

export interface SsoProvider {
    slug: string;
    display_name: string;
}

export const listSsoProviders = () => api<SsoProvider[]>('GET', '/auth/providers');

export function ssoStartUrl(slug: string): string {
    return `${API_BASE}/auth/oidc/${encodeURIComponent(slug)}/start`;
}

export const exchangeSsoCode = (code: string) => api<AuthResponse>('POST', '/auth/oidc/exchange', { code });

export async function changePassword(oldPassword: string, newPassword: string): Promise<string> {
    const [old_password, new_password] = await encryptForServer(oldPassword, newPassword);
    const body = await api<{ token: string }>('POST', '/change-password', { old_password, new_password });
    return body.token;
}

export const validateSession = () => api<UserProfile>('GET', '/validate-session');

export async function changeEmail(newEmail: string, password: string): Promise<void> {
    const [encryptedPassword] = await encryptForServer(password);
    await api<void>('POST', '/change-email', { new_email: newEmail, password: encryptedPassword });
}

export const changeGravatarEmail = (gravatarEmail: string | null) =>
    api<void>('POST', '/change-gravatar-email', { gravatar_email: gravatarEmail });

export async function gravatarUrl(email: string, size = 80): Promise<string> {
    const normalized = email.trim().toLowerCase();
    const encoded = new TextEncoder().encode(normalized);
    const digest = await crypto.subtle.digest('SHA-256', encoded);
    const hash = Array.from(new Uint8Array(digest))
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
    return `https://www.gravatar.com/avatar/${hash}?s=${size}&d=404`;
}

export interface Setup2FAResponse {
    secret: string;
    qr_code: string;
}

export async function setup2FA(password: string): Promise<Setup2FAResponse> {
    const [encryptedPassword] = await encryptForServer(password);
    return api<Setup2FAResponse>('POST', '/2fa/setup', { password: encryptedPassword });
}

export const enable2FA = (code: string) => api<void>('POST', '/2fa/enable', { code });

export const disable2FA = (code: string) => api<void>('POST', '/2fa/disable', { code });

export class TwoFactorRequiredError extends Error {
    constructor(
        public readonly username: string,
        public readonly password: string,
    ) {
        super('2fa_required');
    }
}

export interface SetupStatus {
    state: 'setup_required' | 'ready';
    version: string;
    ca_fingerprint: string | null;
}

export interface SetupInput {
    setupCode: string;
    organizationName: string;
    adminEmail: string;
    password: string;
    consolePin: string;
}

export interface SetupResult extends AuthResponse {
    recovery_key: string;
}

export class SetupError extends Error {
    constructor(
        readonly status: number,
        message: string,
    ) {
        super(message);
    }
}

async function asSetupError<T>(call: () => Promise<T>): Promise<T> {
    try {
        return await call();
    } catch (e) {
        if (e instanceof ApiError) throw new SetupError(e.status, e.message);
        throw e;
    }
}

export const SETUP_CA_URL = `${API_BASE}/setup/ca.crt`;

export const getSetupStatus = () => api<SetupStatus>('GET', '/setup/status');

export async function completeSetup(input: SetupInput): Promise<SetupResult> {
    const [encryptedPassword, encryptedConsolePin] = await encryptForServer(input.password, input.consolePin);
    return asSetupError(() =>
        api<SetupResult>('POST', '/setup', {
            setup_code: input.setupCode,
            organization_name: input.organizationName,
            admin_email: input.adminEmail,
            encrypted_password: encryptedPassword,
            encrypted_console_pin: encryptedConsolePin,
        }),
    );
}

export const requestClusterJoin = (setupCode: string, joinString: string) =>
    asSetupError(() => api<void>('POST', '/setup/join', { setup_code: setupCode, join_string: joinString }));

export interface SshAccess {
    enabled: boolean;
    expires_at: string | null;
    effective: boolean;
}

export const getSshAccess = () => api<SshAccess>('GET', '/admin/settings/ssh');

export const setSshAccess = (enabled: boolean, durationHours: number | null) =>
    api<SshAccess>('PUT', '/admin/settings/ssh', { enabled, duration_hours: durationHours });
