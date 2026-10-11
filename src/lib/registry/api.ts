import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { mock } from './mock.ts';
import type { Repo, Workspace } from './types.ts';

/** Load functions that read the registry depend on this; invalidate it after a change. */
export const REGISTRY = 'app:registry';

// Commands reject with a plain message string.

export async function listWorkspaces(): Promise<Workspace[]> {
	return isTauri() ? invoke('list_workspaces') : mock.listWorkspaces();
}

export async function createWorkspace(name: string): Promise<Workspace> {
	return isTauri() ? invoke('create_workspace', { name }) : mock.createWorkspace(name);
}

export async function addRepo(workspace: string, path: string): Promise<Repo> {
	return isTauri() ? invoke('add_repo', { workspace, path }) : mock.addRepo(workspace, path);
}

export async function removeRepo(workspace: string, repo: string): Promise<void> {
	return isTauri() ? invoke('remove_repo', { workspace, repo }) : mock.removeRepo(workspace, repo);
}

/** Ask for a directory; null when the dialog is dismissed. */
export async function pickDirectory(): Promise<string | null> {
	return isTauri() ? open({ directory: true }) : mock.pickDirectory();
}
