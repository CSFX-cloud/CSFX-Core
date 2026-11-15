import { expect, test } from '@playwright/test';

const TOKEN_KEY = 'csfx_token';
const SESSION_COOKIE = 'auth_token';

test.describe('unauthenticated', () => {
	test('redirects protected pages to the login form', async ({ page }) => {
		await page.goto('/nodes');
		await expect(page).toHaveURL(/\/login\?next=%2Fnodes$/);
		await expect(page.getByRole('heading', { name: 'Sign in.' })).toBeVisible();
	});

	test('answers protected pages with a redirect and no dashboard markup', async ({ request }) => {
		const response = await request.get('/resource-groups', { maxRedirects: 0 });
		expect(response.status()).toBe(303);
		expect(response.headers()['location']).toBe('/login?next=%2Fresource-groups');
		expect(await response.text()).not.toContain('db-shell');
	});

	test('rejects a tampered session cookie', async ({ context }) => {
		await context.addCookies([{ name: SESSION_COOKIE, value: 'forged', url: 'http://localhost:4173' }]);
		const response = await context.request.get('/nodes', { maxRedirects: 0 });
		expect(response.status()).toBe(303);
	});

	test('returns to the requested page after sign in', async ({ page }) => {
		await page.goto('/nodes');
		await page.locator('#username').fill('admin');
		await page.locator('#password').fill('secret');
		await page.getByRole('button', { name: /Sign in/ }).click();
		await expect(page).toHaveURL(/\/nodes$/);
	});

	test('rejects wrong credentials and stays on the login form', async ({ page }) => {
		await page.goto('/login');
		await page.locator('#username').fill('admin');
		await page.locator('#password').fill('wrong');
		await page.getByRole('button', { name: /Sign in/ }).click();
		await expect(page.getByText('Invalid credentials')).toBeVisible();
		await expect(page).toHaveURL(/\/login$/);
		expect(await page.evaluate((key) => localStorage.getItem(key), TOKEN_KEY)).toBeNull();
	});

	test('asks for the second factor when the account requires it', async ({ page }) => {
		await page.goto('/login');
		await page.locator('#username').fill('totp');
		await page.locator('#password').fill('secret');
		await page.getByRole('button', { name: /Sign in/ }).click();
		await expect(page).toHaveURL(/\/otp$/);
	});
});

test.describe('sign in', () => {
	test('stores the session and opens the dashboard', async ({ page }) => {
		await page.goto('/login');
		await page.locator('#username').fill('admin');
		await page.locator('#password').fill('secret');
		await page.getByRole('button', { name: /Sign in/ }).click();
		await expect(page).toHaveURL(/\/$/);
		expect(await page.evaluate((key) => localStorage.getItem(key), TOKEN_KEY)).toBeNull();
		expect(await page.evaluate(() => document.cookie)).not.toContain(SESSION_COOKIE);
		const cookies = await page.context().cookies();
		expect(cookies.find((cookie) => cookie.name === SESSION_COOKIE)?.httpOnly).toBe(true);
	});

	test('sends a signed in user from the login form to the dashboard', async ({ page, context }) => {
		await context.addCookies([{ name: SESSION_COOKIE, value: 'mock-token', url: 'http://localhost:4173' }]);
		await page.goto('/login');
		await expect(page).toHaveURL(/\/$/);
	});

	test('signing out drops the session', async ({ page }) => {
		await page.goto('/login');
		await page.locator('#username').fill('admin');
		await page.locator('#password').fill('secret');
		await page.getByRole('button', { name: /Sign in/ }).click();
		await expect(page).toHaveURL(/\/$/);
		await page.getByRole('button', { name: /admin/ }).click();
		await page.getByRole('menuitem', { name: /Log out/ }).click();
		await expect(page).toHaveURL(/\/login$/);
		await page.goto('/nodes');
		await expect(page).toHaveURL(/\/login\?next=%2Fnodes$/);
	});
});
