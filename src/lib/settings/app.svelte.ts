const THEME = 'vogt:theme';
const LOCALE = 'vogt:locale';

function read(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		// storage is unavailable; the defaults hold
		return null;
	}
}

function write(key: string, value: string): void {
	try {
		localStorage.setItem(key, value);
	} catch {
		// storage is unavailable; the choice lasts for this session only
	}
}

/** The settings that belong to the app rather than to one repository. */
class AppSettings {
	/** The active daisyUI theme; empty follows the system. */
	theme = $state(read(THEME) ?? '');
	/** The interface language; translations arrive with the localisation work. */
	locale = $state(read(LOCALE) ?? 'en');

	setTheme(theme: string): void {
		this.theme = theme;
		write(THEME, theme);
	}

	setLocale(locale: string): void {
		this.locale = locale;
		write(LOCALE, locale);
	}

	/** Puts the theme and the language on the document; call in an effect. */
	apply(): void {
		if (this.theme) document.documentElement.dataset.theme = this.theme;
		else delete document.documentElement.dataset.theme;
		document.documentElement.lang = this.locale;
	}
}

export const appSettings = new AppSettings();
