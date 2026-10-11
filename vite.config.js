import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';

// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from '@sveltejs/adapter-static';

import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { sveltekit } from '@sveltejs/kit/vite';
import process from 'node:process';
const host = process.env.TAURI_DEV_HOST;
// GitHub Actions sets CI; the extra reporters only make sense there
const ci = Boolean(process.env.CI);

// https://vite.dev/config/
export default defineConfig({
	plugins: [
		sveltekit({
			preprocess: vitePreprocess(),
			adapter: adapter({ fallback: 'index.html' })
		})
	],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent Vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port, fail if that port is not available
	server: {
		port: 1420,
		strictPort: true,
		host: host || '127.0.0.1',
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 1421
				}
			: undefined,
		watch: {
			// 3. tell Vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**']
		}
	},
	test: {
		expect: { requireAssertions: true },
		// JUnit is what GitHub Actions test-report actions read; annotations come from 'github-actions'
		reporters: ci ? ['default', 'github-actions', 'junit'] : ['default'],
		outputFile: { junit: 'test-results/junit.xml' },
		coverage: {
			enabled: true,
			provider: 'v8',
			include: ['src/**/*.{js,ts,svelte}'],
			exclude: ['src/**/*.{test,spec,e2e}.{js,ts}', 'src/**/*.d.ts'],
			// lcov, json and json-summary are the formats coverage services and PR-comment actions consume
			reporter: ['text', 'html', 'lcov', 'json', 'json-summary'],
			reportsDirectory: 'coverage',
			reportOnFailure: true
		},
		projects: [
			{
				extends: true,
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: ['src/lib/server/**']
				}
			},

			{
				extends: true,
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
