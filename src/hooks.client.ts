import type { ClientInit } from '@sveltejs/kit/hooks';
import { isTauri } from '@tauri-apps/api/core';
import { initDatabase } from '#lib/db/index.ts';

export const init: ClientInit = async () => {
	// outside the Tauri shell there is no database; the registry answers from a mock
	if (isTauri()) await initDatabase();
};
