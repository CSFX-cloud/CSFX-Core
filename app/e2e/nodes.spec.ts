import { expect, test } from '@playwright/test';

test.beforeEach(async ({ context }) => {
	await context.addInitScript(() => localStorage.setItem('csfx_token', 'mock-token'));
});

test('lists the nodes reported by the api', async ({ page }) => {
	await page.goto('/nodes');
	await expect(page.getByRole('heading', { name: /Nodes\./ })).toBeVisible();
	await expect(page.getByText('fra-node-01')).toBeVisible();
	await expect(page.getByText('fra-node-02')).toBeVisible();
});

test('returns to the login form when the session is no longer valid', async ({ page }) => {
	await page.route('**/api/validate-session', (route) =>
		route.fulfill({ status: 401, json: { error: 'invalid token' } })
	);
	await page.goto('/nodes');
	await expect(page).toHaveURL(/\/login$/);
});
