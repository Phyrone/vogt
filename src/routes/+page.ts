import { resolve } from '$app/paths';
import { error, redirect } from '@sveltejs/kit';
import { m } from '#lib/paraglide/messages.js';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent }) => {
	const { workspaces } = await parent();
	const workspace = workspaces[0];
	// the backend always keeps one
	if (!workspace) error(500, m['error.no_workspace']());
	redirect(307, resolve('/workspaces/[workspace]', { workspace: workspace.id }));
};
