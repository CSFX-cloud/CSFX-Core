import { expect, test } from '@playwright/test';

const TOKEN_KEY = 'csfx_token';

test.describe('unauthenticated', () => {
	test('redirects protected pages to the login form', async ({ page }) => {
		await page.goto('/nodes');
		await expect(page).toHaveURL(/\/login$/);
		await expect(page.getByRole('heading', { name: 'Sign in.' })).toBeVisible();
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
		expect(await page.evaluate((key) => localStorage.getItem(key), TOKEN_KEY)).toBe('mock-token');
	});
});
