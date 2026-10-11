import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ params, parent }) => {
	const { workspaces } = await parent();
	const workspace = workspaces.find((w) => w.id === params.workspace);
	if (!workspace) error(404, 'This workspace no longer exists');
	return { workspace };
};
