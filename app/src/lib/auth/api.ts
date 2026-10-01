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
    const res = await fetch(`${API_BASE}/public-key`);
    if (!res.ok) throw new Error(`public-key fetch failed: ${res.status}`);
    const data = await res.json();
    return data.public_key as string;
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

export async function login(
    username: string,
    password: string,
    twoFactorCode?: string,
): Promise<AuthResponse> {
    const pem = await fetchPublicKeyPem();
    const publicKey = await importRsaKey(pem);
    const encryptedPassword = await encryptPassword(password, publicKey);

    const res = await fetch(`${API_BASE}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            username,
            encrypted_password: encryptedPassword,
            two_factor_code: twoFactorCode ?? null,
        }),
    });

    if (res.status === 403) {
        throw new TwoFactorRequiredError(username, password);
    }

    if (!res.ok) {
        throw new Error(`login failed: ${res.status}`);
    }

    return res.json();
}

export async function changePassword(
    token: string,
    oldPassword: string,
    newPassword: string,
): Promise<void> {
    const pem = await fetchPublicKeyPem();
    const publicKey = await importRsaKey(pem);
    const encryptedOld = await encryptPassword(oldPassword, publicKey);
    const encryptedNew = await encryptPassword(newPassword, publicKey);

    const res = await fetch(`${API_BASE}/change-password`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
            old_password: encryptedOld,
            new_password: encryptedNew,
        }),
    });

    if (!res.ok) throw new Error(`change-password failed: ${res.status}`);
}

export async function validateSession(token: string): Promise<UserProfile> {
    const res = await fetch(`${API_BASE}/validate-session`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error('session invalid');
    return res.json();
}

export async function changeEmail(token: string, newEmail: string): Promise<void> {
    const res = await fetch(`${API_BASE}/change-email`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ new_email: newEmail }),
    });
    if (!res.ok) throw new Error(`change-email failed: ${res.status}`);
}

export async function changeGravatarEmail(token: string, gravatarEmail: string | null): Promise<void> {
    const res = await fetch(`${API_BASE}/change-gravatar-email`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ gravatar_email: gravatarEmail }),
    });
    if (!res.ok) throw new Error(`change-gravatar-email failed: ${res.status}`);
}

export async function gravatarUrl(email: string, size = 80): Promise<string> {
    const normalized = email.trim().toLowerCase();
    const encoded = new TextEncoder().encode(normalized);
    const digest = await crypto.subtle.digest('SHA-256', encoded);
    const hash = Array.from(new Uint8Array(digest))
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
    return `https://www.gravatar.com/avatar/${hash}?s=${size}&d=404`;
}

export type AvatarFallbackStyle = 'initials' | 'blobatar';

export async function getAvatarFallbackStyle(token: string): Promise<AvatarFallbackStyle> {
    const res = await fetch(`${API_BASE}/settings/avatar-fallback`, {
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`get avatar fallback failed: ${res.status}`);
    const data = await res.json();
    return data.style === 'blobatar' ? 'blobatar' : 'initials';
}

export async function setAvatarFallbackStyle(token: string, style: AvatarFallbackStyle): Promise<void> {
    const res = await fetch(`${API_BASE}/settings/avatar-fallback`, {
        method: 'PUT',
        headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ style }),
    });
    if (!res.ok) throw new Error(`set avatar fallback failed: ${res.status}`);
}

export interface Setup2FAResponse {
    secret: string;
    qr_code: string;
}

export async function setup2FA(token: string): Promise<Setup2FAResponse> {
    const res = await fetch(`${API_BASE}/2fa/setup`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(`2fa setup failed: ${res.status}`);
    return res.json();
}

export async function enable2FA(token: string, code: string): Promise<void> {
    const res = await fetch(`${API_BASE}/2fa/enable`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ code }),
    });
    if (!res.ok) throw new Error(`2fa enable failed: ${res.status}`);
}

export async function disable2FA(token: string, code: string): Promise<void> {
    const res = await fetch(`${API_BASE}/2fa/disable`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ code }),
    });
    if (!res.ok) throw new Error(`2fa disable failed: ${res.status}`);
}

export class TwoFactorRequiredError extends Error {
    constructor(
        public readonly username: string,
        public readonly password: string,
    ) {
        super('2fa_required');
    }
}
