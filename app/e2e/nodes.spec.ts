import { expect, test } from '@playwright/test';

const baseUrl = 'http://localhost:4173';

test.beforeEach(async ({ context }) => {
	await context.addCookies([{ name: 'auth_token', value: 'mock-token', url: baseUrl }]);
});

test('lists the nodes reported by the api', async ({ page }) => {
	await page.goto('/nodes');
	await expect(page.getByRole('heading', { name: /Nodes\./ })).toBeVisible();
	await expect(page.getByText('fra-node-01')).toBeVisible();
	await expect(page.getByText('fra-node-02')).toBeVisible();
});

test('returns to the login form when the session is no longer valid', async ({ page, context }) => {
	await context.addCookies([{ name: 'auth_token', value: 'expired', url: baseUrl }]);
	await page.goto('/nodes');
	await expect(page).toHaveURL(/\/login\?next=%2Fnodes$/);
});
