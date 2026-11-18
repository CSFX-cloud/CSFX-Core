import { expect, test, type Page } from '@playwright/test';

const baseUrl = 'http://localhost:4173';

test.beforeEach(async ({ context }) => {
	await context.addCookies([{ name: 'auth_token', value: 'mock-token', url: baseUrl }]);
});

async function openBackups(page: Page) {
	await page.goto('/admin/settings/backups');
	await expect(page.getByRole('heading', { name: /^offsite/ })).toBeVisible();
}

test('lists export targets and recent exports', async ({ page }) => {
	await openBackups(page);
	await expect(page.getByRole('heading', { name: 'Backups' })).toBeVisible();
	await expect(page.getByRole('heading', { name: /^local/ })).toBeVisible();
	await expect(page.getByText('upstream service failed: ceph is not configured')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Restore' }).first()).toBeVisible();
});

test('starts an export and shows it running', async ({ page }) => {
	await openBackups(page);
	await page.getByRole('button', { name: 'New export' }).click();
	await page.getByLabel('Scope').selectOption('config');
	await page.getByRole('button', { name: 'Start export' }).click();
	await expect(page.locator('.cx-badge', { hasText: 'running' })).toBeVisible();
	await expect(page.locator('.cx-badge', { hasText: 'running' })).toHaveCount(0, { timeout: 15_000 });
});

test('adds an encrypted local target only with a matching passphrase', async ({ page }) => {
	await openBackups(page);
	await page.getByRole('button', { name: 'Add target' }).click();
	await page.getByLabel('Name').fill('nas');
	await page.getByLabel('Absolute path').fill('/mnt/nas/csfx');
	await page.getByLabel('Encrypt exports with a passphrase').check();
	await page.getByLabel('Passphrase', { exact: true }).fill('a-long-enough-passphrase');
	await page.getByLabel('Repeat passphrase').fill('something different here');
	await expect(page.getByText('Passphrases do not match')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Save target' })).toBeDisabled();
	await page.getByLabel('Repeat passphrase').fill('a-long-enough-passphrase');
	await page.getByRole('button', { name: 'Save target' }).click();
	await expect(page.getByRole('heading', { name: /^nas/ })).toBeVisible();
});

test('restores a completed export after choosing a target', async ({ page }) => {
	await openBackups(page);
	await page.getByRole('button', { name: 'Restore' }).first().click();
	await expect(page.getByText('Matching configuration rows are overwritten')).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'Restore' }).click();
	await expect(page.getByText('Restore completed')).toBeVisible();
});
