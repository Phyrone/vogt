import { RepoSession } from '#lib/repo/session.svelte.ts';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ params, parent }) => {
	const { workspace } = await parent();
	const repo = workspace.repos.find((r) => r.id === params.repo);
	if (!repo) error(404, 'This repository is no longer in the workspace');
	return { repo, session: new RepoSession() };
};
