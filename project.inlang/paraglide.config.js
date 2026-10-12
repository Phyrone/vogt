import { defineConfig } from '@inlang/paraglide-js';

// The compiler options live here, not in the vite plugin, so that the bundler
// and `paraglide-js compile` (the prepare script, CI) produce the same output.
export default defineConfig({
	outdir: './src/lib/paraglide',
	emitTsDeclarations: true,
	strategy: [
		// overrides everything for tests and embedding
		'globalVariable',
		// the user's own choice, keyed like the other app settings
		'localStorage',
		// the browser's language, which follows the system until a choice is made
		'preferredLanguage',
		'baseLocale'
	],
	localStorageKey: 'vogt:locale'
});
