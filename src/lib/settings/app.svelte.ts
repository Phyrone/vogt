import { PersistedState } from 'runed';
import { z } from 'zod';

/** Every theme daisyUI ships; the interface takes all of its colour from the active one. */
export const THEMES = [
	'light',
	'dark',
	'cupcake',
	'bumblebee',
	'emerald',
	'corporate',
	'synthwave',
	'retro',
	'cyberpunk',
	'valentine',
	'halloween',
	'garden',
	'forest',
	'aqua',
	'lofi',
	'pastel',
	'fantasy',
	'wireframe',
	'black',
	'luxury',
	'dracula',
	'cmyk',
	'autumn',
	'business',
	'acid',
	'lemonade',
	'night',
	'coffee',
	'winter',
	'dim',
	'nord',
	'sunset',
	'caramellatte',
	'abyss',
	'silk'
] as const;

/** The stored choice that follows the system; the document gets no theme attribute then. */
export const SYSTEM = 'system';

/** The themes a stored value may name. */
export const themeChoice = z.enum([...THEMES, SYSTEM]);

/** A theme the document may carry, or {@link SYSTEM}. */
export type Theme = z.infer<typeof themeChoice>;

/** Keeps the stored format a plain string and falls back when storage holds something else. */
function stored<T extends string>(schema: z.ZodType<T>, fallback: T) {
	const safe = schema.catch(fallback);
	return {
		serialize: (value: T) => value,
		deserialize: (value: string) => safe.parse(value)
	};
}

/** The active daisyUI theme, or {@link SYSTEM}. */
export const theme = new PersistedState<Theme>('vogt:theme', SYSTEM, {
	serializer: stored<Theme>(themeChoice, SYSTEM)
});
