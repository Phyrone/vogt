import { beforeEach, describe, expect, test } from 'vitest';

// this runs in the browser project: the states are backed by the real localStorage.
// The module is evaluated once per file, so the defaults are checked before any
// test writes to storage; later reads come from storage itself.
describe('app settings', () => {
	beforeEach(() => {
		localStorage.clear();
	});

	test('starts on the default theme when storage is empty', async () => {
		const { SYSTEM, theme } = await import('./app.svelte.ts');
		expect(theme.current).toBe(SYSTEM);
	});

	test('reads the stored theme', async () => {
		localStorage.setItem('vogt:theme', 'nord');
		const { theme } = await import('./app.svelte.ts');
		expect(theme.current).toBe('nord');
	});

	test('falls back when storage holds something unknown', async () => {
		localStorage.setItem('vogt:theme', 'nrod'); // a typo, or a theme that once existed
		const { SYSTEM, theme } = await import('./app.svelte.ts');
		expect(theme.current).toBe(SYSTEM);

		// an empty string was the old way of saying the system theme
		localStorage.setItem('vogt:theme', '');
		expect(theme.current).toBe(SYSTEM);
	});

	test('remembers a new choice as a plain string', async () => {
		const { theme } = await import('./app.svelte.ts');
		theme.current = 'retro';
		expect(localStorage.getItem('vogt:theme')).toBe('retro');
	});
});
