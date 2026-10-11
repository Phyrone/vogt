import { RepoSession } from '#lib/repo/session.svelte.ts';
import { listWorktrees } from '#lib/repo/worktrees.ts';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ params, parent, url }) => {
	const { workspace } = await parent();
	const repo = workspace.repos.find((r) => r.id === params.repo);
	if (!repo) error(404, 'This repository is no longer in the workspace');
	const worktrees = await listWorktrees(repo.path);
	// ?worktree= holds a path from the listing; without it the registered directory is shown
	const selected = url.searchParams.get('worktree');
	const worktree = worktrees.find((w) => w.path === selected) ?? worktrees.find((w) => w.current);
	return { repo, worktrees, worktree, session: new RepoSession() };
};
