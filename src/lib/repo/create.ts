import { invoke, isTauri } from '@tauri-apps/api/core';

/**
 * Create the directory `name` inside `parent` and start a repository in it.
 * Resolves to the new directory's path; rejects with a message that says what to change.
 */
export async function createRepository(parent: string, name: string): Promise<string> {
	return isTauri() ? invoke('create_repository', { parent, name }) : `${parent}/${name.trim()}`;
}
