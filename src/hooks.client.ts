import type { ClientInit } from '@sveltejs/kit/hooks';
import { isTauri } from '@tauri-apps/api/core';
import { initDatabase } from '#lib/db/index.ts';
import { installMenu } from '#lib/menu.ts';

export const init: ClientInit = async () => {
	// outside the Tauri shell there is no database and no native window menu
	if (!isTauri()) return;
	// a language switch reloads the document, so this runs again and the menu follows the language
	await Promise.all([
		initDatabase(),
		installMenu().catch((reason) => console.error('vogt: no window menu', reason))
	]);
};
