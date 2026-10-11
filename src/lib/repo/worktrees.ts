import { invoke, isTauri } from '@tauri-apps/api/core';

/** One checkout of a repository: the main one, or one added with `git worktree add`. */
export interface Worktree {
	/** The directory's name. */
	name: string;
	path: string;
	/** null while HEAD is detached or has no commit yet. */
	branch: string | null;
	/** The worktree the listing was asked through. */
	current: boolean;
}

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
