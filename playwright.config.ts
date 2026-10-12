import { defineConfig } from '@playwright/test';

const ci = Boolean(process.env.CI);

export default defineConfig({
	// `pnpm build` is the Tauri build; the tests only need the frontend
	webServer: { command: 'pnpm vite:build && pnpm preview', port: 4173, reuseExistingServer: !ci },
	testMatch: '**/*.e2e.{ts,js}',
	forbidOnly: ci,
	// the tests read the interface in English, whatever the machine is set to
	use: { locale: 'en-US' },
	outputDir: 'test-results/playwright',
	// 'github' annotates the run, JUnit and HTML are kept as artifacts
	reporter: ci
		? [
				['github'],
				['junit', { outputFile: 'test-results/playwright-junit.xml' }],
				['html', { open: 'never' }]
			]
		: 'list'
});
