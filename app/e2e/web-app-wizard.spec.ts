import { expect, test } from '@playwright/test';

const baseUrl = 'http://localhost:4173';
const wizardPath = '/resource-groups/g1/add/web-app';
const detection = {
	framework: 'vite',
	mode: 'static',
	install_command: 'npm ci',
	build_command: 'npm run build',
	output_dir: 'dist',
	start_command: null,
	port: null
};

test.beforeEach(async ({ context, page }) => {
	await context.addCookies([{ name: 'auth_token', value: 'mock-token', url: baseUrl }]);
	await page.route('**/api/resource-groups/g1/sites/detect', (route) =>
		route.fulfill({ json: { detection } })
	);
});

async function pullRepository(page: import('@playwright/test').Page) {
	await page.goto(wizardPath);
	await page.locator('#wa-repo').fill('https://github.com/org/shop-web');
	await page.getByRole('button', { name: 'Pull repository' }).click();
	await expect(page.getByText(/Detected/)).toBeVisible();
}

test('shows the three steps and starts on the repository', async ({ page }) => {
	await page.goto(wizardPath);
	const steps = page.getByRole('list', { name: 'Deploy a web app' }).getByRole('listitem');
	await expect(steps).toHaveCount(3);
	await expect(steps.nth(0)).toHaveAttribute('aria-current', 'step');
	await expect(page.getByRole('button', { name: 'Continue' })).toBeDisabled();
});

test('walks through the steps and launches with the chosen build size', async ({ page }) => {
	let created: Record<string, unknown> | null = null;
	await page.route('**/api/resource-groups/g1/sites', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		created = route.request().postDataJSON();
		await route.fulfill({ status: 201, json: { id: 's1' } });
	});
	await pullRepository(page);
	await page.getByRole('button', { name: 'Continue' }).click();
	await expect(page.getByRole('heading', { name: 'Build and run' })).toBeVisible();
	await page.getByRole('button', { name: 'Continue' }).click();
	await expect(page.getByRole('heading', { name: 'Launch' })).toBeVisible();
	await expect(page.getByText('github.com/org/shop-web')).toBeVisible();
	await page.locator('#wa-name').fill('shop-web');
	await page.locator('#wa-size').selectOption('large');
	await page.getByRole('button', { name: 'Deploy' }).click();
	await expect.poll(() => created?.['build_size']).toBe('large');
	expect(created?.['name']).toBe('shop-web');
});

test('going back keeps what was entered', async ({ page }) => {
	await pullRepository(page);
	await page.getByRole('button', { name: 'Continue' }).click();
	await page.getByRole('button', { name: 'Back' }).click();
	await expect(page.locator('#wa-repo')).toHaveValue('https://github.com/org/shop-web');
	await expect(page.getByRole('button', { name: 'Continue' })).toBeEnabled();
});
