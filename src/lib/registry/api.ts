import { isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { database } from './database.ts';
import { mock } from './mock.ts';

/** Load functions that read the registry depend on this; invalidate it after a change. */
export const REGISTRY = 'app:registry';

export const { listWorkspaces, createWorkspace, addRepo, removeRepo } = isTauri() ? database : mock;

/** Ask for a directory; null when the dialog is dismissed. */
export async function pickDirectory(): Promise<string | null> {
	return isTauri() ? open({ directory: true }) : mock.pickDirectory();
}
