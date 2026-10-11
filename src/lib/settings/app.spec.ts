import { afterEach, describe, expect, test, vi } from 'vitest';

/** A stand-in for the webview's storage. */
function fakeStorage(initial: Record<string, string> = {}) {
	const items = new Map(Object.entries(initial));
	return {
		items,
		getItem: (key: string) => items.get(key) ?? null,
		setItem: (key: string, value: string) => void items.set(key, value)
	};
}

describe('app settings', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
		vi.resetModules();
	});

	test('reads the stored theme and language', async () => {
		vi.stubGlobal('localStorage', fakeStorage({ 'vogt:theme': 'nord', 'vogt:locale': 'de' }));
		const { appSettings } = await import('./app.svelte.ts');
		expect(appSettings.theme).toBe('nord');
		expect(appSettings.locale).toBe('de');
	});

	test('remembers a new choice', async () => {
		const storage = fakeStorage();
		vi.stubGlobal('localStorage', storage);
		const { appSettings } = await import('./app.svelte.ts');
		appSettings.setTheme('retro');
		appSettings.setLocale('de');
		expect(appSettings.theme).toBe('retro');
		expect(storage.items.get('vogt:theme')).toBe('retro');
		expect(storage.items.get('vogt:locale')).toBe('de');
	});

	test('falls back to the defaults without storage', async () => {
		vi.stubGlobal('localStorage', undefined);
		const { appSettings } = await import('./app.svelte.ts');
		expect(appSettings.theme).toBe('');
		expect(appSettings.locale).toBe('en');
	});
});
