import { getVersion } from '@tauri-apps/api/app';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { appCacheDir, appConfigDir } from '@tauri-apps/api/path';
import { openPath, openUrl } from '@tauri-apps/plugin-opener';
import { rebuildCaches as rebuildRegistryCaches } from '#lib/registry/api.ts';

/** Whether the page runs in the desktop app, where the actions here reach the system. */
export const desktop = isTauri();

/**
 * Where VOGT keeps the registry and the files around it; null in a plain browser.
 * tauri-plugin-sql puts `vogt.db` in the app config directory.
 */
export async function appDataPath(): Promise<string | null> {
	return desktop ? appConfigDir() : null;
}

/** Where the repository cache files live; null in a plain browser. */
export async function appCachePath(): Promise<string | null> {
	return desktop ? appCacheDir() : null;
}

/** Show the folder that holds the registry in the system file manager. */
export async function openDataDir(): Promise<void> {
	await openPath(await appConfigDir());
}

/** Show the repository cache folder in the system file manager. */
export async function openCacheDir(): Promise<void> {
	await openPath(await appCacheDir());
}

/** Empties every repository cache and lets the indexer build them again. */
export async function rebuildCaches(): Promise<void> {
	// the registry orders it after the syncs on their way, which open the caches it empties
	await rebuildRegistryCaches();
}

/** The version this flavour's update channel offers above the running one, or null when there is none. */
export async function checkForUpdates(): Promise<string | null> {
	if (!desktop) throw 'Update checks need the desktop app';
	return invoke<string | null>('check_update');
}

/** Downloads and installs what the channel offers, then restarts into it. */
export async function installUpdate(): Promise<void> {
	await invoke('install_update');
}

/** The version of the running app; the bundle's version in the desktop app. */
export async function appVersion(): Promise<string> {
	return desktop ? getVersion() : __APP_VERSION__;
}

/** Opens a link in the system browser, or a new tab in a plain browser. */
export async function openExternal(url: string): Promise<void> {
	if (desktop) await openUrl(url);
	else window.open(url, '_blank', 'noopener');
}
