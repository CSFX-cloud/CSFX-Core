import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';

// Redirect to home if already logged in
export const load: PageServerLoad = async ({ cookies }) => {
  const token = cookies.get('auth_token');
  if (token) {
    throw redirect(303, '/');
  }
  return {};
};

export const actions = {
  default: async ({ request, cookies, fetch }) => {
    const data = await request.formData();
    const username = data.get('username');
    const password = data.get('password');

    if (!username || !password) {
      return fail(400, { error: 'Username and password are required' });
    }

    try {
      // Get public key
      const keyResponse = await fetch('http://localhost:8000/api/public-key');
      if (!keyResponse.ok) {
        return fail(500, { error: 'Failed to get encryption key' });
      }
      const { public_key } = await keyResponse.json();

      // Import node-forge for server-side encryption
      const forge = await import('node-forge');
      const publicKeyObj = forge.default.pki.publicKeyFromPem(public_key);
      // Use RSA-OAEP with SHA-256 for encryption
      const encrypted = publicKeyObj.encrypt(password as string, 'RSA-OAEP', {
        md: forge.default.md.sha256.create()
      });
      const encryptedPassword = forge.default.util.encode64(encrypted);

      // Login request
      const loginResponse = await fetch('http://localhost:8000/api/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          username,
          encrypted_password: encryptedPassword,
        }),
      });

      if (!loginResponse.ok) {
        if (loginResponse.status === 401) {
          return fail(401, { error: 'Invalid username or password' });
        }
        return fail(500, { error: 'Login failed' });
      }

      const authData = await loginResponse.json();

      // Set HTTP-only cookie
      cookies.set('auth_token', authData.token, {
        path: '/',
        httpOnly: true,
        sameSite: 'strict',
        secure: false, // Set to true in production with HTTPS
        maxAge: 60 * 60 * 24 * 7, // 7 days
      });

      // Redirect to home
      throw redirect(303, '/');
    } catch (error) {
      if (error instanceof Response) throw error;
      console.error('Login error:', error);
      return fail(500, { error: 'An unexpected error occurred' });
    }
  },
} satisfies Actions;
