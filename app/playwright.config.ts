import { defineConfig, devices } from '@playwright/test';

const apiPort = 8000;
const appPort = 4173;

export default defineConfig({
	testDir: 'e2e',
	fullyParallel: true,
	forbidOnly: Boolean(process.env.CI),
	retries: process.env.CI ? 1 : 0,
	reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',
	use: {
		baseURL: `http://localhost:${appPort}`,
		trace: 'retain-on-failure'
	},
	projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
	webServer: [
		{
			command: 'npm run mock-api',
			env: { MOCK_STATE: 'ready', MOCK_API_PORT: String(apiPort) },
			url: `http://localhost:${apiPort}/api/setup/status`,
			reuseExistingServer: !process.env.CI
		},
		{
			command: `npm run dev -- --port ${appPort} --strictPort`,
			env: { VITE_API_URL: `http://localhost:${apiPort}` },
			url: `http://localhost:${appPort}/login`,
			reuseExistingServer: !process.env.CI,
			timeout: 120_000
		}
	]
});
