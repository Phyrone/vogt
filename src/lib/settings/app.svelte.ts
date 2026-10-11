import { PersistedState } from 'runed';
import { z } from 'zod';
import type { Channel } from '#lib/bindings/Channel.ts';

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

/** The languages the interface will speak; translations arrive with the localisation work. */
export const LANGUAGES = [
	{ id: 'en', name: 'English' },
	{ id: 'de', name: 'Deutsch' }
] as const;

/** The update channels; the Rust side knows where each one's manifest is published. */
export const CHANNELS = [
	{ id: 'stable', name: 'Stable' },
	{ id: 'beta', name: 'Beta' }
] as const satisfies readonly { id: Channel; name: string }[];

/** The themes a stored value may name. */
export const themeChoice = z.enum([...THEMES, SYSTEM]);
/** The languages a stored value may name. */
export const localeChoice = z.enum(LANGUAGES.map((language) => language.id));
/** The update channels a stored value may name. */
export const channelChoice = z.enum(CHANNELS.map((channel) => channel.id));

/** A theme the document may carry, or {@link SYSTEM}. */
export type Theme = z.infer<typeof themeChoice>;
/** A language the interface may be set to. */
export type Locale = z.infer<typeof localeChoice>;

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

/** The interface language. */
export const locale = new PersistedState<Locale>('vogt:locale', 'en', {
	serializer: stored<Locale>(localeChoice, 'en')
});

/** A prerelease was installed from the beta channel, so it stays there until another one is chosen. */
const INSTALLED_FROM: Channel = __APP_VERSION__.includes('-') ? 'beta' : 'stable';

/** The channel the app updates from. */
export const channel = new PersistedState<Channel>('vogt:update-channel', INSTALLED_FROM, {
	serializer: stored<Channel>(channelChoice, INSTALLED_FROM)
});
