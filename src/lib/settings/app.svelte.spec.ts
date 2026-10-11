import { beforeEach, describe, expect, test } from 'vitest';

// this runs in the browser project: the states are backed by the real localStorage.
// The module is evaluated once per file, so the defaults are checked before any
// test writes to storage; later reads come from storage itself.
describe('app settings', () => {
	beforeEach(() => {
		localStorage.clear();
	});

	test('starts on the defaults when storage is empty', async () => {
		const { locale, SYSTEM, theme } = await import('./app.svelte.ts');
		expect(theme.current).toBe(SYSTEM);
		expect(locale.current).toBe('en');
	});

	test('reads the stored theme and language', async () => {
		localStorage.setItem('vogt:theme', 'nord');
		localStorage.setItem('vogt:locale', 'de');
		const { locale, theme } = await import('./app.svelte.ts');
		expect(theme.current).toBe('nord');
		expect(locale.current).toBe('de');
	});

	test('falls back when storage holds something unknown', async () => {
		localStorage.setItem('vogt:theme', 'nrod'); // a typo, or a theme that once existed
		localStorage.setItem('vogt:locale', 'fr');
		const { locale, SYSTEM, theme } = await import('./app.svelte.ts');
		expect(theme.current).toBe(SYSTEM);
		expect(locale.current).toBe('en');

		// an empty string was the old way of saying the system theme
		localStorage.setItem('vogt:theme', '');
		expect(theme.current).toBe(SYSTEM);
	});

	test('remembers a new choice as a plain string', async () => {
		const { locale, theme } = await import('./app.svelte.ts');
		theme.current = 'retro';
		locale.current = 'de';
		expect(localStorage.getItem('vogt:theme')).toBe('retro');
		expect(localStorage.getItem('vogt:locale')).toBe('de');
	});
});
