import { resolve } from '$app/paths';
import { error, redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent }) => {
	const { workspaces } = await parent();
	const workspace = workspaces[0];
	// the backend always keeps one
	if (!workspace) error(500, 'There is no workspace to open');
	redirect(307, resolve('/workspaces/[workspace]', { workspace: workspace.id }));
};
