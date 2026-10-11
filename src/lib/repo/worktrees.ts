import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Worktree } from '#lib/bindings/Worktree.ts';

export type { Worktree };

/** Every worktree of the repository at `path`, the main one first; none if it is not a git repository. */
export async function listWorktrees(path: string): Promise<Worktree[]> {
	if (isTauri()) return invoke('list_worktrees', { path });
	// outside the Tauri shell one mock repository has linked worktrees, the others only their own
	const own = { name: path.split('/').at(-1) ?? path, path, branch: 'main', current: true };
	if (path !== '~/code/vogt') return [own];
	return [
		own,
		{ name: 'vogt-graph', path: '~/code/vogt-graph', branch: 'feat/commit-graph', current: false },
		{ name: 'vogt-review', path: '~/code/vogt-review', branch: null, current: false }
	];
}
