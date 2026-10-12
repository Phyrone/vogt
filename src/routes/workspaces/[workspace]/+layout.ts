import { error } from '@sveltejs/kit';
import { m } from '#lib/paraglide/messages.js';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ params, parent }) => {
	const { workspaces } = await parent();
	const workspace = workspaces.find((w) => w.id === params.workspace);
	if (!workspace) error(404, m['error.workspace_gone']());
	return { workspace };
};
